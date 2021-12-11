mod app;
mod event;
mod worker;
pub use app::{settings, App};
pub use event::{EventRequest, EventResponse, ImageInspectInfo};
pub use worker::DockerWorker;

use anyhow::{Context, Error, Result};
use chrono::{DateTime, Utc};
use clipboard::ClipboardProvider;

pub const APP_NAME: &str = "dockeye";

fn conv_metric(value: f64, unit: &str) -> String {
    const KILO: f64 = 1000.;
    const MEGA: f64 = KILO * KILO;
    const GIGA: f64 = KILO * KILO * KILO;
    const TERA: f64 = KILO * KILO * KILO * KILO;

    let (val, u) = if value < KILO {
        (value, "")
    } else if KILO <= value && value < MEGA {
        (value / KILO, "K")
    } else if MEGA <= value && value < GIGA {
        (value / MEGA, "M")
    } else if GIGA <= value && value < TERA {
        (value / GIGA, "G")
    } else {
        (value / TERA, "T")
    };

    format!("{:.2}{}{}", val, u, unit)
}

pub fn conv_fbs(bytes: f64) -> String {
    conv_metric(bytes, "B/s")
}

pub fn conv_fb(bytes: f64) -> String {
    conv_metric(bytes, "B")
}

pub fn conv_b(bytes: u64) -> String {
    conv_fb(bytes as f64)
}

pub fn format_date(datetime: &DateTime<Utc>) -> String {
    datetime.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn convert_naive_date(secs: i64) -> DateTime<Utc> {
    let naive = chrono::NaiveDateTime::from_timestamp(secs, 0);
    DateTime::from_utc(naive, Utc)
}

fn save_to_clipboard(text: String) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut ctx: clipboard::ClipboardContext = ClipboardProvider::new()?;
    ctx.set_contents(text)
}

/// Converts a memory string like `1G`, `100M` etc. into bytes integer.
pub fn convert_memory(s: &str) -> Result<u64> {
    let mut found_non_dig = None;

    for (i, c) in s.chars().enumerate() {
        if !c.is_numeric() {
            found_non_dig = Some(i);
            break;
        }
    }

    if let Some(i) = found_non_dig {
        let num: u64 = s[..i].parse().context("parsing memory number failed")?;
        let rest = &s[i..];
        match &rest.to_lowercase()[..] {
            "b" => Ok(num),
            "k" => Ok(num.saturating_mul(1000)),
            "m" => Ok(num.saturating_mul(1000).saturating_mul(1000)),
            "g" => Ok(num
                .saturating_mul(1000)
                .saturating_mul(1000)
                .saturating_mul(1000)),
            "t" => Ok(num
                .saturating_mul(1000)
                .saturating_mul(1000)
                .saturating_mul(1000)
                .saturating_mul(1000)),
            _ => Err(Error::msg(format!("invalid unit: {}", rest))),
        }
    } else {
        s.parse().context("parsing memory number failed")
    }
}

#[cfg(not(target_os = "macos"))]
pub const DEFAULT_DOCKER_ADDR: &str = "unix:///var/run/docker.sock";
#[cfg(target_os = "macos")]
pub const DEFAULT_DOCKER_ADDR: &str = "unix:///run/docker.sock";
