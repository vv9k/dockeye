mod app;
mod event;
mod worker;
pub use app::{settings, App};

use chrono::{DateTime, Utc};
use clipboard::ClipboardProvider;
pub use event::{EventRequest, EventResponse, ImageInspectInfo};
pub use worker::DockerWorker;

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

fn save_to_clipboard(text: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx: clipboard::ClipboardContext = ClipboardProvider::new()?;
    ctx.set_contents(text)
}

#[cfg(not(target_os = "macos"))]
pub const DEFAULT_DOCKER_ADDR: &str = "unix:///var/run/docker.sock";
#[cfg(target_os = "macos")]
pub const DEFAULT_DOCKER_ADDR: &str = "unix:///run/docker.sock";
