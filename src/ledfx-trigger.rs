use clap::Parser;
use inotify::{Inotify, WatchMask};
use log::{self, debug, info, warn};
use log::{error, trace};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use opener;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, LockResult, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;
use tray_icon::TrayIconEvent;
use tray_icon::menu::MenuEvent;
use util::led_set_preset;
// use wled_json_api_library::structures::state::State;
// use wled_json_api_library::wled::Wled;
mod config;
mod ledfx;
mod monitor;
mod systray;
mod types;
mod util;
use crate::config::{calc_actual_config_file, load_config};
use crate::ledfx::playpause;
use crate::types::*;
use crate::util::{calc_led_state_scheduled, led_set_brightness, update_wled_cache};

const SERVICE_NAME: &str = "_wled._tcp.local.";
// const NO_SCHEDULE: LEDScheduleSpec = LEDScheduleSpec::None;

fn main() {
    let args = Args::parse();

    // These little mutable beauties get toggled by various events.
    let mut state_ledfx_enabled = true;
    let mut state_self_enabled = true;
    let mut should_die = false;

    let mut inotify = Inotify::init().expect("Failed to initialize inotify");
    let cfgfile = match args.config_path.clone() {
        Some(cfgpath) => cfgpath,
        None => calc_actual_config_file(None),
    };
    inotify
        .watches()
        .add(
            cfgfile,
            WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
        )
        .expect("Failed to add inotify watch");

    let mut svc_config = match load_config(args.config_path.clone()) {
        Ok(config) => config,
        Err(err) => {
            // panic!("Failed to load config: {:?}", err),
            eprintln!("Failed to load config: {:?}", err);
            std::process::exit(-1);
        }
    };

    let tray_svc_config = svc_config.clone();
    let (enabled_send, mut enabled_recv) = tokio::sync::broadcast::channel::<bool>(1);
    let (quit_send, mut quit_recv) = tokio::sync::broadcast::channel::<bool>(1);
    let enabled_send_svcmon = enabled_send.clone();

    if let Some(baseurl) = svc_config.clone().ledfx_url {
        thread::spawn(move || {
            loop {
                if let Ok(actual_state) = ledfx::is_playing(baseurl.as_str()) {
                    info!("Checking enabled state...");
                    info!("Toggling state to: {}", actual_state);
                    if actual_state != state_ledfx_enabled {
                        state_ledfx_enabled = actual_state;
                        // enabled_send_svcmon
                        //     .send(actual_state)
                        //     .expect("Weird failure of internal message: active state broadcast.");
                    }
                }
                info!("State watch sleeping...");
                sleep(Duration::from_secs(5));
            }
        });
    }

    let quit_menu_id = if svc_config.tray_icon {
        info!("Starting up tray icon...");
        systray::launch_taskbar_icon(
            enabled_send.clone(),
            enabled_send.subscribe(),
            quit_send.clone(),
            quit_send.subscribe(),
        )
    } else {
        "NONE".to_string()
    };

    let mut next_ledfx_transition = svc_config.next_ledfx_transition();
    info!("==========================================================");
    info!("= ledfx-trigger booting...");
    info!("==========================================================");
    info!("Loaded config: {:?}", &svc_config);

    util::cfg_logging(svc_config.loglevel, svc_config.logfile.clone());
    let mdns = ServiceDaemon::new().expect("Failed to create daemon");
    info!("Tray icon quit menu id is {}", quit_menu_id);

    // OK, now we setup the monitoring...
    let (_stream, playing_arc) = if let Some(ref audio_config) = svc_config.audio_config {
        let (mon, playing_arc) = monitor::setup_audio(&audio_config).unwrap();
        (Some(mon), playing_arc)
    } else {
        (None, Arc::new(AtomicBool::new(false)))
    }; // Note: Stream has to stay in scope or it gets collected and audio dies.

    let mut quiet_cycles: usize = 0;
    let mut inotify_buffer = [0u8; 4096];
    let mut last_command_by_name: HashMap<String, (f32, Option<u16>, Option<bool>)> =
        HashMap::new();

    loop {
        loop {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                info!("Menu Event {event:?}.. is it: {:?}?", quit_menu_id);
                if event.id.0 == quit_menu_id {
                    should_die = true;
                }
            }
            if let Ok(msg) = quit_recv.try_recv() {
                info!("Quit recv just popped a message! {}", msg);
                should_die = msg;
            }
            if let Ok(enabled) = enabled_recv.try_recv() {
                info!("Got a SELF enabled message of: {}", enabled);
                state_self_enabled = enabled;
            }
            info!("Checking inotify events...");
            if let Ok(events) = inotify.read_events(&mut inotify_buffer) {
                let mut should_break: bool = false;
                for event in events {
                    info!("INOTIFY_EV: {:?}", event);
                    should_break = true;
                }
                if should_break {
                    break;
                }
            }

            let now = std::time::Instant::now();
            if playing_arc.load(Relaxed) {
                debug!("arc says we are playing.");
                quiet_cycles = 0;
            } else {
                debug!("arc says we are quiet.");
                quiet_cycles =
                    (quiet_cycles + 1).min(&svc_config.ledfx_idle_cycles.unwrap_or(3) + 1);
            }
            if let Some(baseurl) = &svc_config.ledfx_url {
                // let mut ledfx_enabled_locked = ledfx_enabled.lock().expect("Failed to unlock");
                debug!("Enabled is set to: {}", state_ledfx_enabled);
                debug!("Got LEDFX url of {}", baseurl);
                if quiet_cycles >= svc_config.ledfx_idle_cycles.unwrap_or(3) || !state_self_enabled
                {
                    // Again, arbitrary
                    debug!("We have been quiet for a couple cycles.");
                    playpause(baseurl.as_str(), true).unwrap_or_else(|_| {
                        warn!("Failed to pause LEDFX!");
                    });
                } else {
                    debug!("We have NOT been quiet for a couple cycles. Showing LEDFX.");
                    playpause(baseurl.as_str(), false).unwrap_or_else(|_| {
                        warn!("Failed to pause LEDFX!");
                    })
                }
            } else {
                debug!("No LEDFX url found. Skipping updates.");
            }

            let today: chrono::DateTime<chrono::Local> = chrono::Local::now();
            let mut leds_ok: usize = 0;
            let mut leds_noconfig: usize = 0;
            let leds_ignore: usize = 0;
            let mut leds_err: usize = 0;
            {
                // Locking die arc...
                if should_die {
                    warn!("I should die now!");
                    break;
                }
            } // Locking die arc...

            //std::thread::sleep(Duration::from_secs(10));
            sleep(Duration::from_secs_f64(svc_config.cycle_seconds));
        } // Loop wleds
        match svc_config.restart_on_cfg_change {
            CfgChangeAction::No => (),
            CfgChangeAction::Exit => {
                info!("Exiting due to a config change.");
                std::process::exit(0);
            }
            CfgChangeAction::Reload => {
                info!("Reloading due to a config change.");
                let old_loglevel = svc_config.loglevel;
                let old_logfile = svc_config.logfile.clone();
                let old_tray_icon = svc_config.tray_icon;
                svc_config = match load_config(args.config_path.clone()) {
                    Ok(config) => config,
                    Err(err) => {
                        // panic!("Failed to load config: {:?}", err),
                        eprintln!("Failed to load config: {:?}", err);
                        std::process::exit(-1);
                    }
                };
                if old_loglevel != svc_config.loglevel
                    || old_logfile != svc_config.logfile
                    || old_tray_icon != svc_config.tray_icon
                {
                    warn!(
                        "Changes to logging and system tray configuration \
                           are ignored when restart_on_cfg_change is 'Reload'."
                    );
                }
            }
        }
        {
            if should_die {
                break;
            }
        } // Locking die arc...
        if should_die {
            break;
        }
    } // Loop inotify
}
