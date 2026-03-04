use chrono::{Datelike, NaiveTime};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::{collections::HashMap, path::PathBuf};
use wled_json_api_library::structures::state::State;
use wled_json_api_library::wled::Wled;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    pub config_path: Option<PathBuf>,
    // /// Number of times to greet
    // #[arg(short, long, default_value_t = 1)]
    // count: u8,
}

#[allow(unused)]
#[derive(Debug)]
pub enum Device {
    Wled(Wled),
    Tasmota,
}

#[derive(Debug)]
pub struct WLED {
    #[allow(unused)]
    pub state: Option<State>,
    pub address: IpAddr,
    pub name: String,
    pub device: Wled,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioConfig {
    #[serde(default = "default_input_device")]
    pub input_device: String,
    #[serde(default = "default_jack")]
    pub jack: bool,
    pub ledfx_threshold_db: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScheduleTime {
    Sunrise,
    SunriseOffset(i16),
    Sunset,
    SunsetOffset(i16),
    Time(chrono::NaiveTime),
}

impl ScheduleTime {
    pub fn to_timestamp(&self, lat: f64, lon: f64) -> u64 {
        let today_date = chrono::Local::now();
        // let tz = today_date.timezone();
        // println!("TZ IS {:?}", tz);
        // let offset = tz
        //     .offset_from_local_datetime(&today_date.naive_local())
        //     .unwrap();
        // println!("Offset is {:?} ({}s)", offset, offset.utc_minus_local());
        // let offset_seconds = offset.utc_minus_local() as u64;
        #[allow(deprecated)]
        let today_date = today_date.date();
        match self {
            Self::Time(time) => {
                let today_date = today_date.and_time(*time).expect("Invalid input time");
                today_date.timestamp() as u64
            }
            Self::Sunrise => {
                let (sunrise_time, _) = sunrise::sunrise_sunset(
                    lat,
                    lon,
                    today_date.year(),
                    today_date.month(),
                    today_date.day(),
                );
                sunrise_time as u64 // + offset_seconds
            }
            Self::SunriseOffset(seconds) => {
                let (sunrise_time, _) = sunrise::sunrise_sunset(
                    lat,
                    lon,
                    today_date.year(),
                    today_date.month(),
                    today_date.day(),
                );
                (sunrise_time + *seconds as i64) as u64 // + offset_seconds
            }
            Self::Sunset => {
                let (_, sunset_time) = sunrise::sunrise_sunset(
                    lat,
                    lon,
                    today_date.year(),
                    today_date.month(),
                    today_date.day(),
                );
                sunset_time as u64 // + offset_seconds
            }
            Self::SunsetOffset(seconds) => {
                let (_, sunset_time) = sunrise::sunrise_sunset(
                    lat,
                    lon,
                    today_date.year(),
                    today_date.month(),
                    today_date.day(),
                );
                (sunset_time + *seconds as i64) as u64 // + offset_seconds
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WLEDChange {
    Brightness(f32),
    Preset(u16),
    Power(bool),
    None,
}

// These are used as a list of times with intensities 0-255.
// We interpolate linearly, and treat each day as a loop, so
// we interpolate between the last time in the previous day,
// and the first time today.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WLEDScheduleItem {
    // pub brightness: f32,
    pub time: ScheduleTime,
    // pub preset: Option<u32>,
    pub change: WLEDChange,
}

impl Default for WLEDScheduleItem {
    fn default() -> Self {
        Self {
            time: ScheduleTime::Time(
                chrono::NaiveTime::from_num_seconds_from_midnight_opt(0, 0)
                    .expect("Total chrono library failure."),
            ),
            change: WLEDChange::None,
        }
    }
}

pub type WLEDSchedule = Vec<WLEDScheduleItem>;

fn default_input_device() -> String {
    "default".to_string()
}

fn default_jack() -> bool {
    false
}

fn default_cycle() -> f64 {
    10.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LEDScheduleSpec {
    Default,
    ByName(String),
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LEDBrightnessConfig {
    pub schedule: LEDScheduleSpec,
    pub min_bri: u8,
    pub max_bri: u8,
}

impl Default for LEDBrightnessConfig {
    fn default() -> LEDBrightnessConfig {
        LEDBrightnessConfig {
            schedule: LEDScheduleSpec::None,
            min_bri: 20,
            max_bri: 128,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub enum CfgChangeAction {
    #[default]
    No,
    Exit,
    Reload,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VisualizationSchedule {
    pub start: ScheduleTime,
    pub end: ScheduleTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LedFxSchedule {
    pub from: ScheduleTime,
    pub until: ScheduleTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub lat: f32,
    pub lon: f32,
    pub leds: HashMap<String, LEDBrightnessConfig>,
    // pub brightnesses: HashMap<String, (u8, u8)>,
    // pub transition_duration: i64, // How long it takes to go full dim from full bright
    pub loglevel: usize, //0: off, 1: error, 2: warn, 3: info, 4: debug, 5: pedantic
    #[serde(default = "default_logfile")]
    pub logfile: Option<PathBuf>,
    pub audio_config: Option<AudioConfig>,
    pub ledfx_url: Option<String>,
    pub ledfx_idle_cycles: Option<usize>,
    pub ledfx_schedule: Option<LedFxSchedule>,
    #[serde(default = "default_cycle")]
    pub cycle_seconds: f64,
    #[serde(default = "default_schedule")]
    pub schedule: HashMap<String, WLEDSchedule>,
    #[serde(default = "default_cfg_change")]
    pub restart_on_cfg_change: CfgChangeAction,
    #[serde(default = "default_tray_icon")]
    pub tray_icon: bool,
    pub bind_address: Option<String>,
    pub vis_schedule: Option<VisualizationSchedule>,
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

impl Config {
    pub fn next_ledfx_transition(&self) -> Option<(ScheduleTime, Option<bool>)> {
        match self.ledfx_schedule.clone() {
            Some(ledfx_schedule) => {
                let from_ts = ledfx_schedule
                    .from
                    .to_timestamp(self.lat as f64, self.lon as f64);
                let until_ts = ledfx_schedule
                    .until
                    .to_timestamp(self.lat as f64, self.lon as f64);
                if chrono::Local::now().timestamp() < from_ts as i64 {
                    Some((ledfx_schedule.from.clone(), Some(true)))
                } else if chrono::Local::now().timestamp() < until_ts as i64 {
                    Some((ledfx_schedule.until.clone(), Some(false)))
                } else {
                    //let tomorrow_midnight = (now + Duration::days(1)).date().and_hms(0, 0, 0);
                    // Some((ScheduleTime::Time(NaiveTime::from_hms(23, 59, 59)), None))
                    Some((
                        ScheduleTime::Time(NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
                        None,
                    ))
                    // No more transitions tonight
                }
            }
            None => None,
        }
    }
}

fn default_cfg_change() -> CfgChangeAction {
    CfgChangeAction::No
}

fn default_tray_icon() -> bool {
    false
}

pub(crate) fn default_schedule() -> HashMap<String, WLEDSchedule> {
    HashMap::from([(
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
    )])
}

fn default_logfile() -> Option<PathBuf> {
    None
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lat: Default::default(),
            lon: Default::default(),
            // exclusions: Default::default(),
            leds: HashMap::new(),
            // brightnesses: Default::default(),
            // transition_duration: Default::default(),
            loglevel: Default::default(),
            logfile: Default::default(),
            audio_config: Default::default(),
            ledfx_url: Default::default(),
            ledfx_idle_cycles: Default::default(),
            ledfx_schedule: Default::default(),
            cycle_seconds: Default::default(),
            schedule: default_schedule(),
            restart_on_cfg_change: default_cfg_change(),
            tray_icon: false,
            bind_address: None,
            vis_schedule: None,
            config_path: None,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::types::ScheduleTime;
    use chrono::Local;

    #[test]
    fn test_scheduletime() {
        let now: chrono::DateTime<chrono::Local> = chrono::Local::now();
        let st = ScheduleTime::Sunrise;
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Sunrise+0000 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);
        println!(
            "Without TZ time is {:?}",
            chrono::DateTime::from_timestamp(st_ts as i64, 0).expect("Invalid datetime")
        );
        println!(
            "My calculated NOW is {:?} and TS is {}",
            now,
            now.timestamp()
        );

        let st = ScheduleTime::SunriseOffset(3600);
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Sunrise+3600 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);

        let st = ScheduleTime::SunriseOffset(-3600);
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Sunrise-3600 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);

        let st = ScheduleTime::Sunset;
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("---\nSunset+0000 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);
        println!(
            "Without TZ that time is {:?}",
            chrono::DateTime::from_timestamp(st_ts as i64, 0).expect("Invalid datetime")
        );
        println!(
            "My calculated NOW is {:?} and TS is {}",
            now,
            now.timestamp()
        );
        println!(
            "That offset would be SUNSET-NOW = {}",
            st_ts as i64 - now.timestamp()
        );
        println!("---");

        let st = ScheduleTime::SunsetOffset(3600);
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Sunset+3600 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);

        let st = ScheduleTime::SunsetOffset(-3600);
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Sunset-3600 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);

        let st = ScheduleTime::Time(chrono::NaiveTime::from_hms(12, 00, 00));
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Naive noon today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);

        let st = ScheduleTime::Time(chrono::NaiveTime::from_hms(21, 50, 00));
        let st_ts = st.to_timestamp(49., -124.);
        let datetime = chrono::DateTime::from_timestamp(st_ts as i64, 0)
            .expect("Invalid datetime")
            .with_timezone(&Local::now().timezone());
        println!("Naive 21:50 today is/was {:?}", datetime);
        println!("That TS is {}", st_ts);
    }
}
