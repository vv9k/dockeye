use anyhow::Result;
use dockeye::{DockerWorker, EventRequest, EventResponse, DEFAULT_DOCKER_ADDR};
use tokio::sync::mpsc;

fn main() -> Result<()> {
    pretty_env_logger::try_init()?;

    let (tx_req, rx_req) = mpsc::channel::<EventRequest>(64);
    let (tx_rsp, rx_rsp) = mpsc::channel::<EventResponse>(64);

    let rt = tokio::runtime::Runtime::new()?;
    let app = dockeye::App::new(tx_req, rx_rsp);
    let native_options = eframe::NativeOptions::default();

    DockerWorker::spawn(rt, DEFAULT_DOCKER_ADDR.to_string(), rx_req, tx_rsp);

    eframe::run_native(Box::new(app), native_options)
}
