use bytes::Bytes;
use docker_api::{
    api::{ContainerId, LogsOpts},
    Docker,
};
use futures::StreamExt;
use log::{debug, error};
use tokio::sync::mpsc;

#[derive(Debug, Default, Clone)]
pub struct Logs(pub Vec<Bytes>);

#[derive(Debug, PartialEq)]
pub enum LogWorkerEvent {
    PollData,
    Kill,
}

#[derive(Debug)]
pub struct LogsWorker {
    pub current_id: ContainerId,
    pub rx_events: mpsc::Receiver<LogWorkerEvent>,
    pub tx_logs: mpsc::Sender<Box<Logs>>,
    pub logs: Box<Logs>,
}

impl LogsWorker {
    pub fn new(
        current_id: impl Into<ContainerId>,
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
        if let Err(e) = self.tx_logs.send(std::mem::take(&mut self.logs)).await {
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
                    if let Some(data) = log_data {
                        match data {
                            Ok(chunk) => {
                            log::trace!("adding chunk");
                                self.logs.0.push(chunk);
                            }
                            Err(e) => {
                                match e {
                                    docker_api::Error::Fault {
                                        code: http::status::StatusCode::NOT_FOUND, message: _
                                    } => break,
                                    e => error!("failed to read container logs: {}", e),
                                }
                            }
                        }
                    } else {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(LogWorkerEvent::PollData) => self.send_logs().await,
                        Some(LogWorkerEvent::Kill) => break,
                        None => continue,

                    }
                }
            }
        }
    }
}
