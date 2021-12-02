mod containers;
mod fonts;
mod images;
pub mod settings;
mod system;
mod ui;

use crate::event::{EventRequest, EventResponse};
use containers::ContainersTab;
use images::ImagesTab;
use settings::{Settings, SettingsWindow};
use system::SystemTab;

use anyhow::{Context, Result};
use docker_api::api::{ContainerDetails, ContainerListOpts, Status};
use eframe::{egui, epi};
use std::collections::VecDeque;
use std::time::SystemTime;
use tokio::sync::mpsc;

pub const SIDE_PANEL_MIN_WIDTH: f32 = 150.;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Tab {
    System,
    Containers,
    Images,
}

impl AsRef<str> for Tab {
    fn as_ref(&self) -> &str {
        match &self {
            Tab::System => "System",
            Tab::Containers => "Containers",
            Tab::Images => "Images",
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::System
    }
}

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,

    update_time: SystemTime,
    current_window: egui::Rect,
    errors: VecDeque<(SystemTime, String)>,

    current_tab: Tab,

    notifications: VecDeque<(SystemTime, String)>,
    containers: ContainersTab,
    images: ImagesTab,
    system: SystemTab,

    settings_window: SettingsWindow,
    popups: VecDeque<ui::ActionPopup>,
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
        self.send_event_notify(EventRequest::ListContainers(Some(
            ContainerListOpts::builder().all(true).build(),
        )));
        self.send_event_notify(EventRequest::ListImages(None));
        self.send_event_notify(EventRequest::SystemInspect);
        self.send_event_notify(EventRequest::SystemDataUsage);
    }

    fn save(&mut self, _storage: &mut dyn epi::Storage) {
        self.save_settings();
    }

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        self.display(ctx);
        self.display_windows(ctx);
    }
}

impl App {
    pub fn display(&mut self, ctx: &egui::CtxRef) {
        if ctx.style().visuals.dark_mode {
            ctx.set_visuals(ui::dark_visuals());
        } else {
            ctx.set_visuals(ui::light_visuals());
        }
        self.current_window = ctx.available_rect();
        self.send_update_request();
        self.read_worker_events();
        self.handle_notifications();
        self.handle_popups();
        self.settings_window.settings.fonts.update_ctx(ctx);

        self.top_panel(ctx);
        self.side_panel(ctx);
        self.central_panel(ctx);
    }

    fn display_windows(&mut self, ctx: &egui::CtxRef) {
        self.settings_window.display(ctx);
    }

    fn top_panel(&mut self, ctx: &egui::CtxRef) {
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *ui::color::D_BG_00
            } else {
                *ui::color::L_BG_0
            },
            margin: egui::vec2(5., 5.),
            ..Default::default()
        };
        egui::TopBottomPanel::top("top_panel")
            .frame(frame)
            .show(ctx, |ui| {
                let tabs = [Tab::System, Tab::Containers, Tab::Images];

                ui.horizontal(|ui| {
                    egui::Grid::new("tab_grid").show(ui, |ui| {
                        for tab in tabs {
                            ui.selectable_value(&mut self.current_tab, tab, tab.as_ref());
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(), |ui| {
                        egui::global_dark_light_mode_switch(ui);

                        if ui.button(ui::icon::SETTINGS).clicked() {
                            self.settings_window.toggle();
                        }
                    });
                });
            });
    }

    #[inline]
    fn side_panel_size(&self) -> f32 {
        (self.current_window.width() / 6.).max(SIDE_PANEL_MIN_WIDTH)
    }

    #[inline]
    fn graph_height(&self) -> f32 {
        (self.current_window.height() / 5.).max(100.)
    }

    fn side_panel(&mut self, ctx: &egui::CtxRef) {
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *ui::color::D_BG_00
            } else {
                *ui::color::L_BG_0
            },
            ..Default::default()
        };
        egui::SidePanel::left("side_panel")
            .frame(frame)
            .min_width(SIDE_PANEL_MIN_WIDTH)
            .max_width(self.side_panel_size())
            .resizable(false)
            .show(ctx, |ui| match self.current_tab {
                Tab::System => self.system_side(ui),
                Tab::Containers => {
                    self.containers_side(ui);
                }
                Tab::Images => {
                    self.images_side(ui);
                }
            });
    }

    fn central_panel(&mut self, ctx: &egui::CtxRef) {
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *ui::color::D_BG_0
            } else {
                *ui::color::L_BG_3
            },
            margin: (10., 10.).into(),
            ..Default::default()
        };
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            self.display_notifications_and_errors(ctx);
            match self.current_tab {
                Tab::System => {
                    self.system_view(ui);
                }
                Tab::Containers => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.containers_view(ui));
                }
                Tab::Images => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.images_view(ui));
                }
            }

            self.display_popups(ctx);
        });
    }

    fn display_notifications_and_errors(&mut self, ctx: &egui::CtxRef) {
        let mut offset = 0.;
        for (_, notification) in &self.notifications {
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
        for (_, error) in &self.errors {
            if let Some(response) = egui::Window::new("Error")
                .id(egui::Id::new(offset as u32))
                .anchor(egui::Align2::RIGHT_TOP, (0., offset))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::RED, error);
                })
            {
                offset += response.response.rect.height();
            }
        }
    }

    fn display_popups(&mut self, ctx: &egui::CtxRef) {
        for popup in &mut self.popups {
            popup.display(ctx);
        }
    }
}

impl App {
    pub fn new(
        settings: Settings,
        tx_req: mpsc::Sender<EventRequest>,
        rx_rsp: mpsc::Receiver<EventResponse>,
    ) -> Self {
        Self {
            tx_req,
            rx_rsp,

            update_time: SystemTime::now(),

            current_tab: Tab::default(),
            current_window: egui::Rect::EVERYTHING,

            errors: VecDeque::new(),
            notifications: VecDeque::new(),
            containers: ContainersTab::default(),
            images: ImagesTab::default(),
            system: SystemTab::default(),

            settings_window: SettingsWindow {
                settings,
                ..Default::default()
            },
            popups: VecDeque::new(),
        }
    }

    fn send_event(&self, event: EventRequest) -> Result<()> {
        self.tx_req.try_send(event).context("sending event failed")
    }

    fn send_event_notify(&mut self, event: EventRequest) {
        if let Err(e) = self.send_event(event).context("sending event failed") {
            self.add_error(e);
        }
    }

    fn add_notification(&mut self, notification: impl std::fmt::Display) {
        self.notifications
            .push_back((SystemTime::now(), format!("{}", notification)));
    }

    fn add_error(&mut self, error: impl std::fmt::Debug) {
        self.errors
            .push_back((SystemTime::now(), format!("{:?}", error)));
    }

    fn send_update_request(&mut self) {
        let elapsed = self.update_time.elapsed().unwrap_or_default().as_millis();
        match self.current_tab {
            Tab::Containers if elapsed > 1000 => {
                self.send_event_notify(EventRequest::ListContainers(Some(
                    ContainerListOpts::builder().all(true).build(),
                )));
                if self.containers.current_container.is_some() {
                    self.send_event_notify(EventRequest::ContainerDetails);
                    self.send_event_notify(EventRequest::ContainerLogs);
                    if self
                        .containers
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
            Tab::Images if elapsed > 1000 => {
                self.send_event_notify(EventRequest::ListImages(None));
                if self.images.pull_view.in_progress || self.images.search_view.pull_in_progress {
                    self.send_event_notify(EventRequest::PullImageChunks);
                }
                self.update_time = SystemTime::now();
            }
            Tab::System if elapsed > 2000 => {
                match self.system.central_view {
                    system::CentralView::Home => {
                        self.send_event_notify(EventRequest::SystemInspect)
                    }
                    system::CentralView::DataUsage => {
                        self.send_event_notify(EventRequest::SystemDataUsage)
                    }
                }
                self.update_time = SystemTime::now();
            }
            _ => {}
        }
    }

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            //log::warn!("[gui] received event: {:?}", event);
            match event {
                EventResponse::ListContainers(mut containers) => {
                    containers.sort_by(|a, b| b.created.cmp(&a.created));
                    self.containers.containers = containers
                }
                EventResponse::ListImages(mut images) => {
                    images.sort_by(|a, b| b.created.cmp(&a.created));
                    self.images.images = images
                }
                EventResponse::ContainerDetails(container) => self.set_container(container),
                EventResponse::InspectContainerNotFound => {
                    self.add_error("container not found");
                    self.containers.clear_container()
                }
                EventResponse::InspectImage(image) => self.images.current_image = Some(image),
                EventResponse::DeleteContainer(res) => match res {
                    Ok(id) => {
                        self.add_notification(format!("successfully deleted container {}", id))
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::DeleteImage(res) => match res {
                    Ok(status) => {
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
                    Err(e) => self.add_error(e),
                },
                EventResponse::ContainerStats(new_stats) => {
                    if let Some(stats) = &mut self.containers.current_stats {
                        stats.extend(*new_stats);
                    } else {
                        self.containers.current_stats = Some(new_stats)
                    }
                }
                EventResponse::ContainerLogs(logs) => {
                    let raw_bytes = logs.0.clone().into_iter().flatten().collect::<Vec<_>>();
                    let escaped_bytes = strip_ansi_escapes::strip(&raw_bytes).unwrap_or(raw_bytes);
                    let logs = String::from_utf8_lossy(&escaped_bytes);
                    if let Some(current_logs) = &mut self.containers.current_logs {
                        current_logs.push_str(&logs);
                    } else {
                        self.containers.current_logs = Some(logs.to_string());
                    }
                }
                EventResponse::StartContainer(res)
                | EventResponse::StopContainer(res)
                | EventResponse::PauseContainer(res)
                | EventResponse::UnpauseContainer(res) => {
                    if let Err(e) = res {
                        self.add_error(e);
                    }
                }
                EventResponse::SaveImage(res) => match res {
                    Ok((id, path)) => self.add_notification(format!(
                        "successfully exported image {} to tar archive in `{}`",
                        id,
                        path.display()
                    )),
                    Err(e) => self.add_error(e),
                },
                EventResponse::PullImage(res) => match res {
                    Ok(id) => {
                        self.images.pull_view.in_progress = false;
                        self.add_notification(format!("successfully pulled image {}", id,))
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::PullImageChunks(new_chunks) => {
                    if let Some(chunks) = &mut self.images.current_pull_chunks {
                        chunks.extend(new_chunks);
                    } else {
                        self.images.current_pull_chunks = Some(new_chunks);
                    }
                }
                EventResponse::DockerUriChange(res) => match res {
                    Ok(()) => {
                        self.clear_all();
                        self.add_notification("Successfully changed Docker uri")
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::ContainerCreate(res) => match res {
                    Ok(id) => {
                        self.add_notification(format!("successfully created container {}", id))
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::SystemInspect(res) => match res {
                    Ok(data) => {
                        self.system.system_info = Some(data);
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::SystemDataUsage(res) => match res {
                    Ok(usage) => {
                        self.system.data_usage = Some(usage);
                    }
                    Err(e) => self.add_error(e),
                },
                EventResponse::ContainerRename(res) => match res {
                    Ok(_) => self.add_notification("successfully renamed a container"),
                    Err(e) => self.add_error(e),
                },
                EventResponse::SearchImage(res) => match res {
                    Ok(mut results) => {
                        results.sort_by(|a, b| b.star_count.cmp(&a.star_count));
                        self.images.search_view.images = Some(results)
                    }
                    Err(e) => self.add_error(e),
                },
            }
        }
    }

    fn handle_notifications(&mut self) {
        loop {
            let should_pop = self
                .notifications
                .front()
                .map(|(time, _)| time.elapsed().unwrap_or_default().as_millis() >= 5000)
                .unwrap_or_default();

            if should_pop {
                self.notifications.pop_front();
            } else {
                break;
            }
        }
        loop {
            let should_pop = self
                .errors
                .front()
                .map(|(time, _)| time.elapsed().unwrap_or_default().as_millis() >= 5000)
                .unwrap_or_default();

            if should_pop {
                self.errors.pop_front();
            } else {
                break;
            }
        }
    }

    fn clear_all(&mut self) {
        self.containers.clear();
        self.images.clear();
    }

    fn set_container(&mut self, container: Box<ContainerDetails>) {
        let changed = self
            .containers
            .current_container
            .as_ref()
            .map(|current| current.id != container.id)
            .unwrap_or(true);

        if changed {
            self.containers.clear_container();
            self.send_event_notify(EventRequest::ContainerTraceStart {
                id: container.id.clone(),
            });
        }

        self.containers.current_container = Some(container);
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings_window.save_settings() {
            self.add_error(e);
        } else {
            self.send_event_notify(EventRequest::DockerUriChange {
                uri: self.settings_window.settings.docker_addr.clone(),
            });
        }
    }

    pub fn docker_uri(&self) -> &str {
        &self.settings_window.settings.docker_addr
    }

    fn handle_popups(&mut self) {
        if let Some(popup) = self.popups.pop_front() {
            if !popup.is_finished() {
                self.popups.push_back(popup);
            } else if popup.is_confirmed() {
                self.send_event_notify(popup.action());
            }
        }
    }
}
