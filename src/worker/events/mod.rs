use crate::worker::WorkerEvent;

use docker_api::{
    api::{Event, EventsOpts},
    Docker,
};
use futures::StreamExt;
use log::{debug, error};
use tokio::sync::mpsc;

use chrono::{DateTime, TimeZone, Utc};

#[derive(Debug)]
pub struct EventsWorker {
    pub rx_events: mpsc::Receiver<WorkerEvent>,
    pub tx_sys_events: mpsc::Sender<Vec<Event>>,
    pub sys_events: Vec<Event>,
    pub last_timestamp: DateTime<Utc>,
}

impl EventsWorker {
    pub fn new() -> (Self, mpsc::Sender<WorkerEvent>, mpsc::Receiver<Vec<Event>>) {
        let (tx_sys_events, rx_sys_events) = mpsc::channel::<Vec<Event>>(128);
        let (tx_events, rx_events) = mpsc::channel::<WorkerEvent>(128);

        (
            Self {
                rx_events,
                tx_sys_events,
                sys_events: vec![],
                last_timestamp: Utc.ymd(1970, 1, 1).and_hms(0, 0, 0),
            },
            tx_events,
            rx_sys_events,
        )
    }
    async fn send_events(&mut self) {
        debug!("got poll data request, sending events");
        if let Err(e) = self
            .tx_sys_events
            .send(std::mem::take(&mut self.sys_events))
            .await
        {
            error!("failed to send system events: {}", e);
        }
    }
    pub async fn work(mut self, docker: Docker) {
        let mut event_stream =
            docker.events(&EventsOpts::builder().since(&self.last_timestamp).build());
        loop {
            tokio::select! {
                event = event_stream.next() => {
                    match event {
                        Some(Ok(event)) => {
                                self.sys_events.push(event);
                            }
                        Some(Err(e)) => {
                            error!("failed to read system events: {}", e);
                        }
                        None => {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
                event = self.rx_events.recv() => {
                    match event {
                        Some(WorkerEvent::PollData) => self.send_events().await,
                        Some(WorkerEvent::Kill) => break,
                        None => continue,

                    }
                }
            }
        }
    }
}
