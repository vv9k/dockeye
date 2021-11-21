mod app;
mod event;
mod stats;
mod worker;
pub use app::App;
pub use event::{EventRequest, EventResponse, ImageInspectInfo};
pub use worker::DockerWorker;

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
