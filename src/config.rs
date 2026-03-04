use crate::types::*;
use anyhow::Result;
/// Manages the configuration; related tools.
use std::collections::HashMap;
use std::path::PathBuf;

fn calc_config_dir() -> PathBuf {
    let mut homedir = dirs::home_dir().expect("Must have a $HOME dir set to run.");
    homedir.push(".wled-doppler");
    homedir
}

fn bootstrap() -> Result<PathBuf> {
    let homedir = calc_config_dir();
    if homedir.is_file() {
        panic!(
            "Configuration dir is a file instead of a dir. {:?}",
            &homedir
        );
    }
    if !homedir.exists() {
        std::fs::create_dir_all(&homedir)?
    }

    let mut cfgpath = homedir.clone();
    cfgpath.push("config.ron");
    if !cfgpath.is_file() {
        let tmpconfig = Config {
            lat: 49.0,
            lon: -124.0,
            // exclusions: Vec::new(),
            // brightnesses: HashMap::new(),
            leds: HashMap::new(),
            // transition_duration: 3600i64,
            loglevel: 4,
            logfile: None,
            audio_config: None,
            ledfx_url: None,
            ledfx_idle_cycles: Some(3),
            cycle_seconds: 10.0,
            schedule: HashMap::from([(
                "default".to_string(),
                vec![
                    WLEDScheduleItem {
                        time: ScheduleTime::Sunrise,
                        change: WLEDChange::Brightness(1.0),
                    },
                    WLEDScheduleItem {
                        time: ScheduleTime::Sunset,
                        change: WLEDChange::Brightness(0.2),
                    },
                ],
            )]),
            restart_on_cfg_change: CfgChangeAction::No,
            tray_icon: false,
            bind_address: Some("localhost:3178".to_string()),
            vis_schedule: None,
            config_path: Some(cfgpath.clone()),
            ledfx_schedule: Default::default(),
        };
        let cfgstr = ron::ser::to_string_pretty(&tmpconfig, ron::ser::PrettyConfig::default())
            .expect("Wups, my default config is borked?!");
        std::fs::write(&cfgpath, cfgstr.as_bytes())?
    }

    Ok(cfgpath)
}

pub fn calc_actual_config_file(cfg_path: Option<PathBuf>) -> PathBuf {
    match cfg_path {
        Some(tmp_path) => tmp_path,
        None => {
            let mut tmp_path = calc_config_dir();
            tmp_path.push("config.ron");
            tmp_path
        }
    }
}

pub fn load_config(cfg_path: Option<PathBuf>) -> Result<Config> {
    let cfgdir = match cfg_path {
        Some(cfgpath) => cfgpath,
        None => bootstrap()?,
    };
    let cfgfile = std::fs::read_to_string(&cfgdir)?;
    let mut cfg: Config = ron::de::from_bytes(cfgfile.as_bytes())?;
    cfg.config_path = Some(cfgdir.clone());
    Ok(cfg)
}
