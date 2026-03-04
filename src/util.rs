use crate::types::*;
use anyhow::{Result, anyhow};
use chrono::Datelike;
use fern::colors::{Color, ColoredLevelConfig};
use fern::log_file;
use log::{self, error, info, trace, warn};
use mdns_sd::ServiceInfo;
use reqwest::Url;
use std::collections::HashMap;
use std::path::PathBuf;
use wled_json_api_library::structures::state::State;
use wled_json_api_library::wled::Wled;

pub(crate) fn cfg_logging(level: usize, log_path: Option<PathBuf>) {
    let levels = [
        log::LevelFilter::Off,
        log::LevelFilter::Error,
        log::LevelFilter::Warn,
        log::LevelFilter::Info,
        log::LevelFilter::Debug,
        log::LevelFilter::Trace,
    ];
    configure_logging(
        *levels.get(level).unwrap_or(&log::LevelFilter::Info),
        log_path,
    );
}

fn configure_logging(loglevel: log::LevelFilter, logfile: Option<PathBuf>) {
    // Configure logger at runtime
    println!("Configuring logging...");
    let colors = ColoredLevelConfig::new()
        .info(Color::Green)
        .warn(Color::Yellow)
        .error(Color::Red)
        .debug(Color::Magenta);
    let fernlog = fern::Dispatch::new()
        .level(loglevel)
        .level_for("mdns_sd", log::LevelFilter::Warn)
        .level_for("reqwest", log::LevelFilter::Warn)
        .chain(
            fern::Dispatch::new()
                // Perform allocation-free log formatting
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{} {} {}] {}",
                        humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
                        colors.color(record.level()),
                        record.target(),
                        message
                    ))
                })
                .chain(std::io::stdout()),
        );

    let fernlog = if let Some(logpath) = logfile {
        info!("Logging to '{:?}'", logpath);
        fernlog.chain(
            fern::Dispatch::new()
                // Perform allocation-free log formatting
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{} {} {}] {}",
                        humantime::format_rfc3339(std::time::SystemTime::now()),
                        record.level(),
                        record.target(),
                        message
                    ))
                })
                .level(loglevel)
                .level_for("mdns_sd", log::LevelFilter::Warn)
                .level_for("reqwest", log::LevelFilter::Warn)
                .chain(
                    log_file(&logpath)
                        .unwrap_or_else(|_| panic!("Could not use log file path {:?}", &logpath)),
                ),
        )
    } else {
        fernlog
    };

    fernlog.apply().unwrap();
}

/// Set the brightness of the given wled device.
#[allow(unused)]
pub fn led_set_preset(wled: &mut WLED, new_preset: u16) -> Result<()> {
    wled.device.state = Some(State {
        on: None,
        bri: None,
        transition: None,
        tt: None,
        ps: Some(new_preset as i32),
        psave: None,
        pl: None,
        nl: None,
        udpn: None,
        v: None,
        rb: None,
        live: None,
        lor: None,
        time: None,
        mainseg: None,
        playlist: None,
        seg: None,
    });
    match wled.device.flush_state() {
        Ok(response) => {
            trace!(
                "    - HTTP response: {:?}",
                response.text().unwrap_or("UNKNOWN ERROR".to_string())
            );
            Ok(())
        }
        Err(err) => {
            error!(
                "    - Failed to update WLED: '{}' with error: {:?}",
                &wled.name, err
            );
            Err(anyhow!(
                "Failed to update wled {} with error {:?}",
                &wled.name,
                err
            ))
        }
    }
}

#[allow(unused)]
pub fn led_set_power(wled: &mut WLED, power: bool) -> Result<()> {
    wled.device.state = Some(State {
        on: Some(power),
        ..Default::default()
    });
    match wled.device.flush_state() {
        Ok(response) => {
            trace!(
                "    - HTTP response: {:?}",
                response.text().unwrap_or("UNKNOWN ERROR".to_string())
            );
            Ok(())
        }
        Err(err) => {
            error!(
                "    - Failed to update WLED: '{}' with error: {:?}",
                &wled.name, err
            );
            Err(anyhow!(
                "Failed to update wled {} with error {:?}",
                &wled.name,
                err
            ))
        }
    }
}

/// Set the brightness of the given wled device.
#[allow(unused)]
pub fn led_set_brightness(wled: &mut WLED, new_bri: u8) -> Result<()> {
    wled.device.state = Some(State {
        on: if new_bri > 0 { Some(true) } else { Some(false) },
        bri: Some(new_bri),
        transition: None,
        tt: None,
        ps: None,
        psave: None,
        pl: None,
        nl: None,
        udpn: None,
        v: None,
        rb: None,
        live: None,
        lor: None,
        time: None,
        mainseg: None,
        playlist: None,
        seg: None,
    });
    match wled.device.flush_state() {
        Ok(response) => {
            trace!(
                "    - HTTP response: {:?}",
                response.text().unwrap_or("UNKNOWN ERROR".to_string())
            );
            Ok(())
        }
        Err(err) => {
            error!(
                "    - Failed to update WLED: '{}' with error: {:?}",
                &wled.name, err
            );
            Err(anyhow!(
                "Failed to update wled {} with error {:?}",
                &wled.name,
                err
            ))
        }
    }
}

#[allow(unused)]
pub fn update_wled_cache(info: &ServiceInfo, found_wled: &mut HashMap<String, WLED>) -> Result<()> {
    let full_name = info.get_fullname().to_string();
    let short_name = info.get_hostname().to_string();
    let old_wled = found_wled.get(&full_name);
    if old_wled.is_none()
        || (old_wled.is_some() && !info.get_addresses().contains(&old_wled.unwrap().address))
    {
        if old_wled.is_some() {
            warn!("WLED '{}' may have changed IPs. Updating.", full_name);
        }
        // let ip_addr: Option<IpAddr> = None;
        for try_ip in info.get_addresses() {
            let url: Url =
                Url::try_from(format!("http://{}:{}/", try_ip, info.get_port()).as_str())
                    .unwrap_or_else(|_| {
                        panic!("Invalid addr/port: {}:{}", try_ip, info.get_port())
                    });
            info!("Found WLED {} at: {}", &short_name, &url);
            let mut wled: Wled = Wled::try_from_url(&url).unwrap();
            // info!("new wled: {wled:?}");
            match wled.get_state_from_wled() {
                Ok(()) => {
                    if let Some(state) = &wled.state {
                        // info!("WLED CFG: {:?}", &wled.cfg);
                        found_wled.insert(
                            full_name.to_string(),
                            WLED {
                                state: Some(state.clone()),
                                address: *try_ip,
                                name: info.get_fullname().to_string(),
                                device: wled,
                            },
                        );
                        return Ok(());
                    }
                }
                Err(the_error) => {
                    warn!(
                        "Failed to read config from WLED: {} -> {}",
                        full_name, the_error
                    );
                }
            }
        }
        return Err(anyhow!("Could not register WLED: {}", info.get_fullname()));
    }

    Ok(())
}

/// Calculates how much we should dim (from 0.0 as no dimming, to 1.0 as fully dimmed)
/// based on what time of day it is. Contains much magic (of the black datetime variety).
#[allow(unused)]
pub fn calc_dim_pc(
    today: chrono::DateTime<chrono::Local>,
    lat: f64,
    lon: f64,
    transition_duration: i64,
) -> f32 {
    // OK, Let's now calculate
    let today_date = today.date_naive();
    let (sunrise_time, sunset_time) = sunrise::sunrise_sunset(
        lat,
        lon,
        today_date.year(),
        today_date.month(),
        today_date.day(),
    );
    info!("Sunrise, Sunset: {:?}, {:?}", sunrise_time, sunset_time);
    info!(
        "Current unix time: {} and sunset is in {} seconds",
        today.timestamp(),
        sunset_time - today.timestamp()
    );
    if today.timestamp() > (sunrise_time + transition_duration)
        && today.timestamp() <= (sunset_time - transition_duration)
    {
        info!("No dim yet, still daytime");
        0.
    } else if today.timestamp() > (sunset_time - transition_duration)
        && today.timestamp() < sunset_time
    {
        info!("Twilight.");
        // OK, we're dimming
        (today.timestamp() - (sunset_time - transition_duration)) as f32
            / transition_duration as f32
    } else if today.timestamp() >= sunset_time {
        info!("MAX DIM; It's late.");
        1.
    } else if today.timestamp() <= sunrise_time {
        info!("MAX DIM; It's really fucking early.");
        1.
    } else if today.timestamp() > sunrise_time
        && today.timestamp() < sunrise_time + transition_duration
    {
        info!("Calc morning unDIM; It's early AM after sunrise.");
        1. - (today.timestamp() as f32 - sunrise_time as f32) / transition_duration as f32
    } else {
        // Fallback to super dim so we don't blind anybody if we escape those clausese^^
        1.
    }
}

#[allow(unused)]
pub(crate) fn calc_led_state_scheduled(
    now: chrono::DateTime<chrono::Local>,
    lat: f64,
    lon: f64,
    schedule: &Vec<WLEDScheduleItem>,
) -> (f32, Option<u16>, Option<bool>) {
    // Generate initial event lists.
    let mut bri_ev: Vec<(u64, f32)> = schedule
        .clone()
        .iter()
        .filter(|i| matches!(i.change, WLEDChange::Brightness(_)))
        .map(|i| {
            (
                i.time.to_timestamp(lat, lon),
                if let WLEDChange::Brightness(bri) = i.change {
                    bri
                } else {
                    0.
                },
            )
        })
        .collect();
    let mut pre_ev: Vec<(u64, u16)> = schedule
        .clone()
        .iter()
        .filter(|i| matches!(i.change, WLEDChange::Preset(_)))
        .map(|i| {
            (
                i.time.to_timestamp(lat, lon),
                if let WLEDChange::Preset(pre) = i.change {
                    pre
                } else {
                    panic!("WTF?! Unreachable preset.");
                    1 // Note, this is hopefully unreachable.
                },
            )
        })
        .collect();
    let mut power_ev: Vec<(u64, bool)> = schedule
        .clone()
        .iter()
        .filter(|i| matches!(i.change, WLEDChange::Power(_)))
        .map(|i| {
            (
                i.time.to_timestamp(lat, lon),
                if let WLEDChange::Power(power) = i.change {
                    power
                } else {
                    panic!("WTF?! Unreachable preset.");
                    false // Note, this is hopefully unreachable.
                },
            )
        })
        .collect();

    // Add offsets to beginning and end...
    let mut tmp_bri_ev = bri_ev.clone();
    let mut tmp_pre_ev = pre_ev.clone();
    if let Some(last) = bri_ev.last() {
        tmp_bri_ev.insert(0, (last.0 - (24 * 3600), last.1));
    }
    if let Some(first) = bri_ev.first() {
        tmp_bri_ev.push((first.0 + (24 * 3600), first.1));
    }
    bri_ev = tmp_bri_ev;
    if let Some(last) = pre_ev.last() {
        tmp_pre_ev.insert(0, (last.0 - (24 * 3600), last.1));
    }
    if let Some(first) = pre_ev.first() {
        tmp_pre_ev.push((first.0 + (24 * 3600), first.1));
    }
    pre_ev = tmp_pre_ev;

    // Same for presets

    // Then we determine current time and where that sits.
    let now: i64 = now.timestamp();

    if bri_ev.len() < 2 || pre_ev.len() == 1 {
        // Note that an event length of "1" is the only invalid
        // preset config. No changes is fine, and more than 1 is always valid.
        error!("Invalid schedule found: {:?}.", &schedule);
        return (0., None, None); // This is fucked. Should always have a couple entries.
    }

    let mut out: f32 = 0.;

    for i in 0..(bri_ev.len() - 1) {
        let before = bri_ev
            .get(i)
            .expect("Failed to index into a known position.");
        let after = bri_ev
            .get(i + 1)
            .expect("Failed to index into a known position.");
        if now >= before.0 as i64 && now <= after.0 as i64 {
            let delta_pc = (now as f32 - before.0 as f32) / (after.0 as f32 - before.0 as f32);
            let delta_amt = after.1 - before.1;
            out = before.1 + delta_pc * delta_amt;

            break;
        }
    }

    let mut preset_out: Option<u16> = None;
    if pre_ev.len() >= 2 {
        for i in 0..(pre_ev.len() - 1) {
            let before = pre_ev
                .get(i)
                .expect("Failed to index into a known position.");
            let after = pre_ev
                .get(i + 1)
                .expect("Failed to index into a known position.");
            if now >= before.0 as i64 && now <= after.0 as i64 {
                preset_out = Some(before.1);
                break;
            }
        }
    }

    let mut power_out: Option<bool> = None;

    (out, preset_out, power_out)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{ScheduleTime, WLEDChange, WLEDSchedule};
    use chrono::{DateTime, Datelike, Local, NaiveTime};
    use fern::colors::{Color, ColoredLevelConfig};

    #[test]
    fn test_calc_dimming_schedule() {
        let dispatch = fern::Dispatch::new();
        let colors = ColoredLevelConfig::new().debug(Color::Magenta);
        let fernlog = dispatch
            .level(log::LevelFilter::Trace)
            .level_for("mdns_sd", log::LevelFilter::Warn)
            .level_for("reqwest", log::LevelFilter::Warn)
            .chain(
                fern::Dispatch::new()
                    // Perform allocation-free log formatting
                    .format(move |out, message, record| {
                        out.finish(format_args!(
                            "[{} {} {}] {}",
                            humantime::format_rfc3339_seconds(std::time::SystemTime::now()),
                            colors.color(record.level()),
                            record.target(),
                            message
                        ))
                    })
                    .chain(std::io::stdout())
                    .chain(std::io::stderr()),
            );
        info!("Test output via logging...");
        let today: chrono::DateTime<chrono::Local> = chrono::Local::now();
        let simple_dimming_schedule = WLEDSchedule::from([
            WLEDScheduleItem {
                time: ScheduleTime::Time(NaiveTime::from_hms(7, 0, 0)),
                change: WLEDChange::Brightness(0.2),
            },
            WLEDScheduleItem {
                time: ScheduleTime::Time(NaiveTime::from_hms(8, 0, 0)),
                change: WLEDChange::Brightness(0.8),
            },
            WLEDScheduleItem {
                time: ScheduleTime::Time(NaiveTime::from_hms(19, 0, 0)),
                change: WLEDChange::Brightness(0.8),
            },
            WLEDScheduleItem {
                time: ScheduleTime::Time(NaiveTime::from_hms(20, 0, 0)),
                change: WLEDChange::Brightness(0.2),
            },
        ]);

        let dim_pc = calc_led_state_scheduled(today, 49., -124., &simple_dimming_schedule);
        println!("DIM PC actual: {:?}", dim_pc);

        let dim_pc = calc_led_state_scheduled(
            today.with_time(NaiveTime::from_hms(19, 30, 0)).unwrap(),
            49.,
            -124.,
            &simple_dimming_schedule,
        );

        println!("DIM PC at 7:30PM: {:?}", dim_pc);
        let dim_pc = calc_led_state_scheduled(
            today.with_time(NaiveTime::from_hms(7, 30, 0)).unwrap(),
            49.,
            -124.,
            &simple_dimming_schedule,
        );
        println!("DIM PC at 7:30AM: {:?}", dim_pc);

        let dim_pc = calc_led_state_scheduled(
            today.with_time(NaiveTime::from_hms(0, 0, 0)).unwrap(),
            49.,
            -124.,
            &simple_dimming_schedule,
        );
        println!("DIM PC at midnight: {:?}", dim_pc);
    }

    #[test]
    fn test_calc_dimming() {
        let today: chrono::DateTime<chrono::Local> = chrono::Local::now();
        let today_date = today.date_naive();
        let (sunrise_time, sunset_time) = sunrise::sunrise_sunset(
            49_f64,
            -124_f64,
            today_date.year(),
            today_date.month(),
            today_date.day(),
        );
        let dim_pc = calc_dim_pc(today, 49., -124., 1200);
        info!("DIM PC: {}", dim_pc);
        let sunset_dt = DateTime::from_timestamp(sunset_time, 0).unwrap();
        let sunset_dt: DateTime<Local> = sunset_dt.into();

        let dim_pc = calc_dim_pc(sunset_dt, 49., -124., 1200);
        info!("DIM PC at sunset: {}", dim_pc);
    }

    #[test]
    fn test_bri_calc() {
        let high = 50u8;
        let low = 1u8;
        let gap = (high - low) as f32;
        let dim_pc = 1.0f32;
        let new_bri = (high as f32 - (dim_pc * gap)).min(255.).max(0.) as u8;
        info!("New bri is {}", new_bri);
    }
}
