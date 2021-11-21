mod containers;
mod images;

use crate::event::{EventRequest, EventResponse, ImageInspectInfo};
use crate::stats::StatsWrapper;
use anyhow::{Context, Result};
use docker_api::api::{ContainerDetails, ContainerInfo, ContainerListOpts, ImageInfo, Status};
use eframe::{egui, epi};
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

const PACKAGE_ICON: &str = "\u{1F4E6}";
const SCROLL_ICON: &str = "\u{1F4DC}";
const INFO_ICON: &str = "\u{2139}";
const DELETE_ICON: &str = "\u{1F5D9}";
const PLAY_ICON: &str = "\u{25B6}";
const PAUSE_ICON: &str = "\u{23F8}";
const STOP_ICON: &str = "\u{23F9}";

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Tab {
    Containers,
    Images,
}

impl AsRef<str> for Tab {
    fn as_ref(&self) -> &str {
        match &self {
            Tab::Containers => "Containers",
            Tab::Images => "Images",
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::Containers
    }
}

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,

    update_time: SystemTime,
    notifications_time: SystemTime,

    current_tab: Tab,

    notifications: VecDeque<String>,

    containers: Vec<ContainerInfo>,
    current_container: Option<Box<ContainerDetails>>,
    current_stats: Option<Vec<(Duration, StatsWrapper)>>,
    images: Vec<ImageInfo>,
    current_image: Option<Box<ImageInspectInfo>>,
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
        self.handle_data_update();
        self.read_worker_events();
        self.handle_notifications();

        self.top_panel(ctx);
        self.side_panel(ctx);
        self.central_panel(ctx);
    }
}

impl App {
    fn top_panel(&mut self, ctx: &egui::CtxRef) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let tabs = [Tab::Containers, Tab::Images];

            egui::Grid::new("tab_grid").show(ui, |ui| {
                for tab in tabs {
                    ui.selectable_value(&mut self.current_tab, tab, tab.as_ref());
                }
            });
        });
    }

    fn side_panel(&mut self, ctx: &egui::CtxRef) {
        egui::SidePanel::left("side_panel")
            .min_width(150.)
            .show(ctx, |ui| match self.current_tab {
                Tab::Containers => {
                    self.containers_scroll(ui);
                }
                Tab::Images => {
                    self.image_scroll(ui);
                }
            });
    }

    fn central_panel(&mut self, ctx: &egui::CtxRef) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.display_notifications(ctx);
            match self.current_tab {
                Tab::Containers => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.container_details(ui));
                }
                Tab::Images => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.image_details(ui));
                }
            }
        });
    }

    fn display_notifications(&self, ctx: &egui::CtxRef) {
        let mut offset = 0.;
        for notification in &self.notifications {
            if let Some(response) = egui::Window::new("Notification")
                .id(egui::Id::new(offset as u32))
                .anchor(egui::Align2::RIGHT_TOP, (0., offset))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(&notification);
                })
            {
                offset += response.response.rect.height();
            }
        }
    }
}

impl App {
    pub fn new(tx_req: mpsc::Sender<EventRequest>, rx_rsp: mpsc::Receiver<EventResponse>) -> Self {
        Self {
            tx_req,
            rx_rsp,

            update_time: SystemTime::now(),
            notifications_time: SystemTime::now(),

            current_tab: Tab::default(),

            notifications: VecDeque::new(),

            containers: vec![],
            current_container: None,
            current_stats: None,
            images: vec![],
            current_image: None,
        }
    }

    fn send_event(&self, event: EventRequest) -> Result<()> {
        self.tx_req.try_send(event).context("sending event failed")
    }

    fn send_event_notify(&mut self, event: EventRequest) {
        if let Err(e) = self.send_event(event).context("sending event failed") {
            self.add_notification(e);
        }
    }

    fn add_notification(&mut self, notification: impl std::fmt::Display) {
        self.notifications.push_back(format!("{}", notification));
    }

    fn send_update_request(&mut self) {
        self.send_event_notify(EventRequest::ListContainers(Some(
            ContainerListOpts::builder().all(true).build(),
        )));
        self.send_event_notify(EventRequest::ListImages(None));
        if self.current_container.is_some() {
            self.send_event_notify(EventRequest::InspectContainer {
                id: self
                    .current_container
                    .as_ref()
                    .map(|c| c.id.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        if self
            .current_container
            .as_ref()
            .map(|c| containers::is_running(c))
            .unwrap_or_default()
        {
            self.send_event_notify(EventRequest::ContainerStats);
        }
        self.update_time = SystemTime::now();
    }

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            match event {
                EventResponse::ListContainers(containers) => self.containers = containers,
                EventResponse::ListImages(images) => self.images = images,
                EventResponse::InspectContainer(container) => self.set_container(container),
                EventResponse::InspectImage(image) => self.current_image = Some(image),
                EventResponse::DeleteContainer(msg) => self.add_notification(msg),
                EventResponse::DeleteImage(status) => {
                    let status = status.into_iter().fold(String::new(), |mut acc, s| {
                        match s {
                            Status::Deleted(s) => {
                                acc.push_str("Deleted: ");
                                acc.push_str(&s)
                            }
                            Status::Untagged(s) => {
                                acc.push_str("Untagged: ");
                                acc.push_str(&s)
                            }
                        }
                        acc.push('\n');
                        acc
                    });
                    self.add_notification(status)
                }
                EventResponse::ContainerStats(stats) => self.current_stats = Some(stats),
                EventResponse::StartContainer(res)
                | EventResponse::StopContainer(res)
                | EventResponse::PauseContainer(res)
                | EventResponse::UnpauseContainer(res) => {
                    if let Err(e) = res {
                        self.add_notification(e);
                    }
                }
            }
        }
    }

    fn pop_notification(&mut self) {
        self.notifications.pop_front();
        self.notifications_time = SystemTime::now();
    }

    fn handle_notifications(&mut self) {
        if self
            .notifications_time
            .elapsed()
            .unwrap_or_default()
            .as_secs()
            > 5
        {
            self.pop_notification();
        }
    }

    fn handle_data_update(&mut self) {
        if self.update_time.elapsed().unwrap_or_default().as_millis() > 1000 {
            self.send_update_request();
        }
    }

    fn set_container(&mut self, container: Box<ContainerDetails>) {
        let changed = self
            .current_container
            .as_ref()
            .map(|current| current.id != container.id)
            .unwrap_or(true);

        if changed {
            self.current_stats = None;
            if containers::is_running(&container) {
                if let Err(e) = self.send_event(EventRequest::ContainerStatsStart {
                    id: container.id.clone(),
                }) {
                    self.add_notification(e);
                }
            }
        }

        self.current_container = Some(container);
    }
}
