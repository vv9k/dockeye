use anyhow::Result;
use bytes::Bytes;
use docker_api::{api::LogsOpts, Docker};
use futures::StreamExt;
use log::{debug, error};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Logs(pub Vec<Bytes>);

#[derive(Debug, PartialEq)]
pub enum LogWorkerEvent {
    WantData,
    Kill,
}

#[derive(Debug)]
pub struct LogsWorker {
    pub current_id: String,
    pub rx_events: mpsc::Receiver<LogWorkerEvent>,
    pub tx_logs: mpsc::Sender<Box<Logs>>,
    pub logs: Box<Logs>,
}

impl LogsWorker {
    pub fn new(
        current_id: impl Into<String>,
    ) -> (
        Self,
        mpsc::Sender<LogWorkerEvent>,
        mpsc::Receiver<Box<Logs>>,
    ) {
        let (tx_logs, rx_logs) = mpsc::channel::<Box<Logs>>(128);
        let (tx_events, rx_events) = mpsc::channel::<LogWorkerEvent>(128);

        (
            Self {
                current_id: current_id.into(),
                rx_events,
                tx_logs,
                logs: Box::new(Logs(vec![])),
            },
            tx_events,
            rx_logs,
        )
    }
    async fn send_logs(&mut self) {
        debug!("got poll data request, sending logs");
        if let Err(e) = self.tx_logs.send(self.logs.clone()).await {
            error!("failed to send container logs: {}", e);
        }
    }
    pub async fn work(mut self, docker: Docker) {
        let container = docker.containers().get(&self.current_id);
        let mut logs_stream = container.logs(
            &LogsOpts::builder()
                .stderr(true)
                .stdout(true)
                .follow(true)
                .all()
                .build(),
        );
        loop {
            tokio::select! {
                log_data = logs_stream.next() => {
                    log::trace!("got data {:?}", log_data);
                    if let Some(data) = log_data {
                        match data {
                            Ok(chunk) => {
                                self.logs.0.push(chunk);
                            }
                            Err(e) => {
                                error!("reading chunk failed: {}", e);
                            }
                        }
                    } else {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(LogWorkerEvent::WantData) => self.send_logs().await,
                        Some(LogWorkerEvent::Kill) => break,
                        None => continue,

                    }
                }
            }
        }
    }
}
