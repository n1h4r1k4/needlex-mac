//! Weather and calendar for the island's extra pages.
//!
//! * Weather: where the Mac is (Location Services, through the helper, kept
//!   for an hour and rounded to ~1 km) → the forecast from Open-Meteo (free,
//!   no account), kept for 15 minutes. The page shows now, today's high / low
//!   and the next hours.
//! * Calendar: today's and tomorrow's events from the Calendar app (through
//!   the helper); they never leave the Mac.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::{helper, AppState};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hour {
    /// local time, "2026-11-15T18:00"
    time: String,
    temp: f64,
    code: i64,
    is_day: bool,
    /// chance of rain, %
    rain: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Weather {
    city: String,
    temp: f64,
    code: i64,
    is_day: bool,
    high: f64,
    low: f64,
    /// local time of the reading, "2026-11-15T17:45"
    now: String,
    /// the next 12 hours
    hours: Vec<Hour>,
}

#[derive(Default)]
pub struct Cache {
    place: Option<(Instant, f64, f64, String)>,
    weather: Option<(Instant, Weather)>,
}

pub type WeatherCache = Mutex<Cache>;

/// Blocking (the helper may be waiting for the permission prompt).
fn location(app: &AppHandle) -> Result<(f64, f64, String), String> {
    let state = app.state::<AppState>();
    if let Some((at, lat, lon, city)) = state.weather.lock().unwrap().place.clone() {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok((lat, lon, city));
        }
    }
    // the first time macOS asks for permission – give the user time to answer
    let line = helper::ask("loc", Duration::from_secs(90))?;
    let mut it = line.splitn(3, ' ');
    let lat: f64 = it.next().and_then(|v| v.parse().ok()).ok_or("bad location")?;
    let lon: f64 = it.next().and_then(|v| v.parse().ok()).ok_or("bad location")?;
    let city = it.next().unwrap_or("").trim().to_string();
    state.weather.lock().unwrap().place = Some((Instant::now(), lat, lon, city.clone()));
    Ok((lat, lon, city))
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

#[tauri::command]
pub async fn get_weather(app: AppHandle) -> Result<Weather, String> {
    let (cached, http) = {
        let state = app.state::<AppState>();
        let cached = state.weather.lock().unwrap().weather.clone();
        (cached, state.http.clone())
    };
    if let Some((at, w)) = cached {
        if at.elapsed() < Duration::from_secs(15 * 60) {
            return Ok(w);
        }
    }
    let a = app.clone();
    let (lat, lon, city) = tauri::async_runtime::spawn_blocking(move || location(&a))
        .await
        .map_err(|e| e.to_string())??;
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.2}&longitude={lon:.2}\
         &current=temperature_2m,weather_code,is_day\
         &hourly=temperature_2m,weather_code,precipitation_probability,is_day\
         &daily=temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=2"
    );
    let v: Value = http
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let cur = &v["current"];
    let now = cur["time"].as_str().unwrap_or("").to_string();
    let hourly = &v["hourly"];
    let times = hourly["time"].as_array().cloned().unwrap_or_default();
    // the next 12 hours, starting with the one after the current reading
    let this_hour = now.get(..13).unwrap_or("").to_string();
    let first = times.iter().position(|t| t.as_str().map(|t| t.get(..13).unwrap_or("") > this_hour.as_str()).unwrap_or(false));
    let hours = first
        .map(|i| {
            (i..(i + 12).min(times.len()))
                .map(|k| Hour {
                    time: times[k].as_str().unwrap_or("").to_string(),
                    temp: num(&hourly["temperature_2m"][k]),
                    code: hourly["weather_code"][k].as_i64().unwrap_or(0),
                    is_day: hourly["is_day"][k].as_i64().unwrap_or(1) == 1,
                    rain: num(&hourly["precipitation_probability"][k]),
                })
                .collect()
        })
        .unwrap_or_default();
    let w = Weather {
        city,
        temp: num(&cur["temperature_2m"]),
        code: cur["weather_code"].as_i64().unwrap_or(0),
        is_day: cur["is_day"].as_i64().unwrap_or(1) == 1,
        high: num(&v["daily"]["temperature_2m_max"][0]),
        low: num(&v["daily"]["temperature_2m_min"][0]),
        now,
        hours,
    };
    app.state::<AppState>().weather.lock().unwrap().weather = Some((Instant::now(), w.clone()));
    Ok(w)
}

/// Today's and tomorrow's events: [{title, start, end (ms), allDay, color}].
#[tauri::command]
pub async fn get_calendar() -> Result<Value, String> {
    let line = tauri::async_runtime::spawn_blocking(|| helper::ask("cal", Duration::from_secs(90)))
        .await
        .map_err(|e| e.to_string())??;
    serde_json::from_str(&line).map_err(|e| e.to_string())
}
