use crate::{EventRequest, EventResponse};
use docker_api::api::{ContainerInfo, ImageInfo};
use eframe::{egui, epi};
use std::time::SystemTime;
use tokio::sync::mpsc;

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,
    containers: Vec<ContainerInfo>,
    images: Vec<ImageInfo>,
    update_time: SystemTime,
}

impl epi::App for App {
    fn name(&self) -> &str {
        "dockeye"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
    }

    fn save(&mut self, _storage: &mut dyn epi::Storage) {}

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        if self.update_time.elapsed().unwrap_or_default().as_millis() > 500 {
            self.send_update_request();
        }
        self.read_worker_events();
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {});

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            for container in &self.containers {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(&container.id[..12]);
                        ui.add_space(2.);
                        ui.label(&container.image);
                    });
                });
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {});
    }
}

impl App {
    pub fn new(tx_req: mpsc::Sender<EventRequest>, rx_rsp: mpsc::Receiver<EventResponse>) -> Self {
        Self {
            tx_req,
            rx_rsp,
            containers: vec![],
            images: vec![],
            update_time: SystemTime::now(),
        }
    }

    fn send_update_request(&mut self) {
        self.tx_req.try_send(EventRequest::ListContainers(None));
        self.tx_req.try_send(EventRequest::ListImages(None));
        self.update_time = SystemTime::now();
    }

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            match event {
                EventResponse::ListContainers(containers) => self.containers = containers,
                EventResponse::ListImages(images) => self.images = images,
            }
        }
    }
}
