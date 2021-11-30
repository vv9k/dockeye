use anyhow::Result;
use dockeye::{
    settings::{self, Settings},
    DockerWorker, EventRequest, EventResponse,
};
use tokio::sync::mpsc;

fn main() -> Result<()> {
    pretty_env_logger::try_init()?;

    let (tx_req, rx_req) = mpsc::channel::<EventRequest>(64);
    let (tx_rsp, rx_rsp) = mpsc::channel::<EventResponse>(64);

    let rt = tokio::runtime::Runtime::new()?;
    let settings = settings::dir()
        .and_then(|p| {
            let p = p.join(settings::FILENAME);
            match Settings::load(&p) {
                Ok(settings) => Some(settings),
                Err(e) => {
                    log::error!("failed to read settings from `{}`: {}", p.display(), e);
                    None
                }
            }
        })
        .unwrap_or_default();

    let app = dockeye::App::new(settings, tx_req, rx_rsp);
    let native_options = eframe::NativeOptions {
        initial_window_size: Some((1280., 720.).into()),
        ..Default::default()
    };
    let uri = app.docker_uri().to_string();

    DockerWorker::spawn(rt, uri, rx_req, tx_rsp);

    eframe::run_native(Box::new(app), native_options)
}
