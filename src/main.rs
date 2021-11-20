use anyhow::Result;
use dockeye::{DockerWorker, EventRequest, EventResponse};
use std::thread;
use tokio::sync::mpsc;

fn main() -> Result<()> {
    pretty_env_logger::try_init()?;

    let (tx_req, rx_req) = mpsc::channel::<EventRequest>(64);
    let (tx_rsp, rx_rsp) = mpsc::channel::<EventResponse>(64);

    let app = dockeye::App::new(tx_req, rx_rsp);
    let native_options = eframe::NativeOptions::default();

    thread::spawn(move || -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        let worker = DockerWorker::new("unix:///var/run/docker.sock", rx_req, tx_rsp)?;

        rt.block_on(worker.work())
    });

    eframe::run_native(Box::new(app), native_options)
}
