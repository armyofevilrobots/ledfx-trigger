use inotify::{EventMask, Inotify, WatchMask};
use log::error;
use log::{debug, info, warn};
use std::io;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;
// use wled_json_api_library::structures::state::State;
// use wled_json_api_library::wled::Wled;
use crate::config::load_config;
use crate::types::*;

pub(crate) fn inotify_worker(
    config_path: PathBuf,
    mut config_send: tokio::sync::broadcast::Sender<Config>,
    mut die_recv: tokio::sync::broadcast::Receiver<bool>,
) {
    let svc_config = match load_config(Some(config_path.clone())) {
        Ok(config) => config,
        Err(err) => {
            // panic!("Failed to load config: {:?}", err),
            panic!("Failed to load config: {:?}", err);
        }
    };
    let mut inotify = Inotify::init().expect("Failed to initialize inotify");
    info!("Setting up inotify.");
    let mut inotify_buffer = [0u8; 4096];
    inotify
        .watches()
        .add(
            config_path.clone(),
            WatchMask::MODIFY | WatchMask::CREATE, //| WatchMask::DELETE,
        )
        .expect("Failed to add inotify watch");

    loop {
        info!("iNotify watching...");
        match die_recv.try_recv() {
            Ok(_) => {
                info!("iNotify worker: Graceful shutdown.");
                return;
            }
            Err(err) => match err {
                TryRecvError::Closed => {
                    error!("iNotify worker: Watchdog is dead!");
                    return;
                }
                TryRecvError::Empty => debug!("iNotify worker: No shutdown yet!"),
                _ => warn!("Unexpected result from watchdog?"),
            },
        };
        match inotify.read_events(&mut inotify_buffer) {
            Ok(events) => {
                println!("EVENTS FOUND.");
                for event in events {
                    match load_config(Some(config_path.clone())) {
                        Ok(config) => {
                            // Yes, I know, we're ignoring a potential failure.
                            info!("Sending new config!");
                            config_send.send(config).unwrap_or_else(|_err| {
                                error!("Failed to send new config!");
                                0
                            });
                            // This is such a weird way to say "Hey, I deleted your watchmask entry."
                            if event.mask.contains(EventMask::IGNORED) {
                                info!("Recreating watch mask due to deletion.");
                                inotify
                                    .watches()
                                    .add(
                                        config_path.clone(),
                                        WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
                                    )
                                    .expect("Failed to add inotify watch");
                            }
                            // Old watch is already gone because the file was deleted and replaced.
                        }
                        Err(err) => {
                            // panic!("Failed to load config: {:?}", err),
                            error!("Failed to load config: {:?}", err);
                        }
                    };
                }
            }
            Err(err) => {
                debug!(
                    "Failed to get new events. No events==11? {:?}",
                    err.raw_os_error()
                );
            }
        }
        sleep(Duration::from_millis(1000));
    }
}
