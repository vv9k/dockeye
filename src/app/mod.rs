mod containers;
mod images;

use crate::event::{EventRequest, EventResponse, ImageInspectInfo};
use crate::worker::RunningContainerStats;
use anyhow::{Context, Result};
use docker_api::api::{ContainerDetails, ContainerInfo, ContainerListOpts, ImageInfo, Status};
use eframe::{egui, epi};
use std::collections::VecDeque;
use std::time::SystemTime;
use tokio::sync::mpsc;

const PACKAGE_ICON: &str = "\u{1F4E6}";
const SCROLL_ICON: &str = "\u{1F4DC}";
const INFO_ICON: &str = "\u{2139}";
const DELETE_ICON: &str = "\u{1F5D9}";
const PLAY_ICON: &str = "\u{25B6}";
const PAUSE_ICON: &str = "\u{23F8}";
const STOP_ICON: &str = "\u{23F9}";
const SETTINGS_ICON: &str = "\u{2699}";

macro_rules! key {
    ($ui:ident, $k:expr) => {
        $ui.add(Label::new($k).strong());
    };
}
macro_rules! val {
    ($ui:ident, $v:expr) => {
        if $ui
            .add(Label::new($v).monospace().sense(egui::Sense {
                click: true,
                focusable: true,
                drag: false,
            }))
            .on_hover_text("secondary-click to copy")
            .secondary_clicked()
        {
            log::debug!("setting clipboard content to `{}`", $v);
            if let Err(e) = crate::save_to_clipboard($v.to_string()) {
                log::error!("failed to save content to clipboard - {}", e);
            }
        }
    };
}
macro_rules! key_val {
    ($ui:ident, $k:expr, $v:expr) => {
        key!($ui, $k);
        val!($ui, $v);
        $ui.end_row();
    };
}

pub(crate) use {key, key_val, val};

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

#[derive(Default, Debug)]
pub struct SettingsWindow {
    show: bool,
    config: Config,
}

impl SettingsWindow {
    pub fn toggle(&mut self) {
        self.show = !self.show;
    }

    pub fn display(&mut self, ctx: &egui::CtxRef) {
        egui::Window::new("settings")
            .open(&mut self.show)
            .show(ctx, |ui| {
                ui.label("Docker address:");
                ui.text_edit_singleline(&mut self.config.docker_addr);
            });
    }
}

#[derive(Debug)]
pub struct Config {
    docker_addr: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            docker_addr: crate::DEFAULT_DOCKER_ADDR.to_string(),
        }
    }
}

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,

    update_time: SystemTime,
    notifications_time: SystemTime,
    current_window: egui::Rect,
    errors: VecDeque<String>,

    current_tab: Tab,

    notifications: VecDeque<String>,

    containers: Vec<ContainerInfo>,
    current_container: Option<Box<ContainerDetails>>,
    current_stats: Option<Box<RunningContainerStats>>,
    current_logs: Option<String>,
    images: Vec<ImageInfo>,
    current_image: Option<Box<ImageInspectInfo>>,

    logs_page: usize,

    settings_window: SettingsWindow,
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
        self.display(ctx);
        self.display_windows(ctx);
    }
}

impl App {
    pub fn display(&mut self, ctx: &egui::CtxRef) {
        self.current_window = ctx.available_rect();
        self.handle_data_update();
        self.read_worker_events();
        self.handle_notifications();

        self.top_panel(ctx);
        self.side_panel(ctx);
        self.central_panel(ctx);
    }

    fn display_windows(&mut self, ctx: &egui::CtxRef) {
        self.settings_window.display(ctx);
    }

    fn top_panel(&mut self, ctx: &egui::CtxRef) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let tabs = [Tab::Containers, Tab::Images];

            egui::Grid::new("tab_grid").show(ui, |ui| {
                for tab in tabs {
                    ui.selectable_value(&mut self.current_tab, tab, tab.as_ref());
                }
            });
            ui.with_layout(egui::Layout::right_to_left(), |ui| {
                if ui.button(SETTINGS_ICON).clicked() {
                    self.settings_window.toggle();
                }
            });
        });
    }

    #[inline]
    fn side_panel_size(&self) -> f32 {
        (self.current_window.width() / 6.).max(100.)
    }

    #[inline]
    fn graph_height(&self) -> f32 {
        (self.current_window.height() / 5.).max(100.)
    }

    fn side_panel(&mut self, ctx: &egui::CtxRef) {
        egui::SidePanel::left("side_panel")
            .min_width(100.)
            .max_width(250.)
            .max_width(self.side_panel_size())
            .resizable(false)
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
            current_window: egui::Rect::EVERYTHING,

            errors: VecDeque::new(),
            notifications: VecDeque::new(),

            containers: vec![],
            current_container: None,
            current_stats: None,
            current_logs: None,
            images: vec![],
            current_image: None,
            logs_page: 0,

            settings_window: SettingsWindow::default(),
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

    fn add_error(&mut self, error: impl std::fmt::Display) {
        self.errors.push_back(format!("{}", error));
    }

    fn send_update_request(&mut self) {
        self.send_event_notify(EventRequest::ListContainers(Some(
            ContainerListOpts::builder().all(true).build(),
        )));
        self.send_event_notify(EventRequest::ListImages(None));
        if self.current_container.is_some() {
            self.send_event_notify(EventRequest::ContainerDetails);
            self.send_event_notify(EventRequest::ContainerLogs);
            if self
                .current_container
                .as_ref()
                .map(|c| containers::is_running(c))
                .unwrap_or_default()
            {
                self.send_event_notify(EventRequest::ContainerStats);
            }
        }
        self.update_time = SystemTime::now();
    }

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            //log::warn!("[gui] received event: {:?}", event);
            match event {
                EventResponse::ListContainers(containers) => self.containers = containers,
                EventResponse::ListImages(images) => self.images = images,
                EventResponse::ContainerDetails(container) => self.set_container(container),
                EventResponse::InspectContainerNotFound => {
                    self.add_error("container not found");
                    self.clear_container()
                }
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
                EventResponse::ContainerStats(new_stats) => {
                    if let Some(stats) = &mut self.current_stats {
                        stats.extend(*new_stats);
                    } else {
                        self.current_stats = Some(new_stats)
                    }
                }
                EventResponse::ContainerLogs(logs) => {
                    let bytes = logs.0.clone().into_iter().flatten().collect::<Vec<_>>();
                    let raw_bytes = strip_ansi_escapes::strip(&bytes).unwrap_or(bytes);
                    let logs = String::from_utf8_lossy(&raw_bytes);
                    if let Some(current_logs) = &mut self.current_logs {
                        current_logs.push_str(&logs);
                    } else {
                        self.current_logs = Some(logs.to_string());
                    }
                }
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

    fn handle_notifications(&mut self) {
        if self
            .notifications_time
            .elapsed()
            .unwrap_or_default()
            .as_millis()
            >= 5000
        {
            self.notifications.pop_front();
            self.errors.pop_front();
            self.notifications_time = SystemTime::now();
        }
    }

    fn handle_data_update(&mut self) {
        if self.update_time.elapsed().unwrap_or_default().as_millis() > 1000 {
            self.send_update_request();
        }
    }

    fn clear_container(&mut self) {
        self.current_container = None;
        self.current_stats = None;
        self.current_logs = None;
        self.logs_page = 0;
    }

    fn set_container(&mut self, container: Box<ContainerDetails>) {
        let changed = self
            .current_container
            .as_ref()
            .map(|current| current.id != container.id)
            .unwrap_or(true);

        if changed {
            self.clear_container();
            if let Err(e) = self.send_event(EventRequest::ContainerTraceStart {
                id: container.id.clone(),
            }) {
                self.add_notification(e);
            }
        }

        self.current_container = Some(container);
    }
}
