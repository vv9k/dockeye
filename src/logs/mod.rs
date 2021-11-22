use anyhow::Result;
use docker_api::{api::LogsOpts, conn::TtyChunk, Docker};
use futures::StreamExt;
use log::{debug, error};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Logs(pub Vec<TtyChunk>);

#[derive(Debug)]
pub struct LogsWorker {
    pub docker: Docker,
    pub rx_id: mpsc::Receiver<String>,
    pub rx_want_data: mpsc::Receiver<()>,
    pub tx_logs: mpsc::Sender<Box<Logs>>,
}

impl LogsWorker {
    pub async fn work(self) -> Result<()> {
        let Self {
            mut rx_id,
            tx_logs,
            mut rx_want_data,
            docker,
        } = self;
        log::debug!("[logs-worker] starting...");
        loop {
            let mut current_id = None;
            let mut logs = Box::new(Logs(vec![]));
            //let mut last = SystemTime::now();

            tokio::select! {
                id = rx_id.recv() => {
                    if let Some(id) = id {
                        if Some(&id) != current_id.as_ref() {
                            current_id = Some(id.to_string());
                            logs.0.clear();
                        }
                        let container = docker.containers().get(&id);
                        let mut logs_stream = container.logs(&LogsOpts::builder().stderr(true).stdout(true).follow(true).build());
                        loop {
                            tokio::select! {
                                log_data = logs_stream.next() => {
                                    debug!("[logs-worker] got logs chunk {:?}", log_data);
                                    if let Some(data) = log_data {
                                        match data {
                                            Ok(chunk) => {
                                                logs.0.push(chunk);
                                            }
                                            Err(e) => {
                                                error!("[logs-worker] reading chunk failed: {}", e);
                                            }
                                        }
                                    }
                                }
                                //_ = rx_want_data.recv() => {
                                    //debug!("[logs-worker] got poll data request, sending logs");
                                    //if let Err(e) = tx_logs.send(logs.clone()).await {
                                        //error!("[logs-worker] failed to send container logs: {}", e);
                                    //}
                                //}
                                _id = rx_id.recv() => if let Some(_id) = _id {
                                    if Some(&_id) != current_id.as_ref() {
                                        debug!("[logs-worker] received new id: {}", _id);
                                        current_id = Some(_id);
                                        logs.0.clear();
                                    }
                                }
                            }
                        }
                    }
                }
                _ = rx_want_data.recv() => {
                    debug!("[logs-worker] got poll data request, sending logs");
                    if let Err(e) = tx_logs.send(logs.clone()).await {
                        error!("[logs-worker] failed to send container logs: {}", e);
                    }
                }
            }
        }
    }
}
