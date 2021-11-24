use anyhow::Result;
use bytes::Bytes;
use docker_api::{api::LogsOpts, Container, Docker};
use futures::StreamExt;
use log::{debug, error};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Logs(pub Vec<Bytes>);

#[derive(Debug)]
pub struct LogsWorker<'d> {
    pub rx_id: mpsc::Receiver<String>,
    pub rx_want_data: mpsc::Receiver<()>,
    pub tx_logs: mpsc::Sender<Box<Logs>>,
    pub current_id: Option<String>,
    pub container: Option<Container<'d>>,
    pub logs: Box<Logs>,
}

impl<'d> LogsWorker<'d> {
    pub fn new() -> (
        Self,
        mpsc::Sender<String>,
        mpsc::Sender<()>,
        mpsc::Receiver<Box<Logs>>,
    ) {
        let (tx_logs, rx_logs) = mpsc::channel::<Box<Logs>>(128);
        let (tx_want_data, rx_want_data) = mpsc::channel::<()>(128);
        let (tx_id, rx_id) = mpsc::channel::<String>(128);

        (
            Self {
                rx_id,
                rx_want_data,
                tx_logs,
                current_id: None,
                container: None,
                logs: Box::new(Logs(vec![])),
            },
            tx_id,
            tx_want_data,
            rx_logs,
        )
    }
    fn set_container(&mut self, docker: &'d Docker, id: &str) -> bool {
        if Some(id) != self.current_id.as_deref() {
            debug!("[stats-worker] changing container to {}", id);
            self.current_id = Some(id.to_string());
            self.logs.0.clear();
            self.container = Some(docker.containers().get(id));
            return true;
        }
        false
    }
    async fn send_logs(&mut self) {
        debug!("[logs-worker] got poll data request, sending logs");
        if let Err(e) = self.tx_logs.send(self.logs.clone()).await {
            error!("[logs-worker] failed to send container logs: {}", e);
        }
    }
    async fn inner_work(&mut self, docker: &'d Docker, id: Option<String>) {
        if let Some(id) = id {
            self.set_container(&docker, &id);
            let mut container = self.container.as_ref().unwrap();
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
                                    self.logs.0.push(chunk);
                                }
                                Err(e) => {
                                    error!("[logs-worker] reading chunk failed: {}", e);
                                }
                            }
                        }
                    }
                    _ = self.rx_want_data.recv() => self.send_logs().await,
                    _id = self.rx_id.recv() => if let Some(_id) = _id {
                        if self.set_container(&docker, &_id) {
                            container = self.container.as_ref().unwrap();
                            logs_stream = container.logs(
                                &LogsOpts::builder()
                                    .stderr(true)
                                    .stdout(true)
                                    .follow(true)
                                    .all()
                                    .build(),
                            );

                        }
                    }
                }
            }
        }
    }

    pub async fn work(mut self, docker: Docker) -> Result<()> {
        log::debug!("[logs-worker] starting...");
        loop {
            tokio::select! {
                id = self.rx_id.recv() => self.inner_work(&docker, id).await,
                _ = self.rx_want_data.recv() => self.send_logs().await,
            }
        }
    }
}
