mod containers;
mod fonts;
mod images;
mod networks;
pub mod settings;
mod system;
mod ui;
mod volumes;

use crate::event::{
    ContainerEvent, ContainerEventResponse, EventRequest, EventResponse, GuiEventResponse,
    ImageEvent, ImageEventResponse, NetworkEvent, NetworkEventResponse, VolumeEvent,
    VolumeEventResponse,
};
use containers::ContainersTab;
use images::ImagesTab;
use networks::NetworksTab;
use settings::{Settings, SettingsWindow};
use system::SystemTab;
use volumes::VolumesTab;

use anyhow::{Context, Result};
use docker_api::api::{ContainerDetails, ContainerListOpts, Status};
use docker_api::conn::TtyChunk;
use eframe::egui;
use egui::style::Margin;
use std::collections::VecDeque;
use std::time::SystemTime;
use tokio::sync::mpsc;

pub const SIDE_PANEL_MIN_WIDTH: f32 = 150.;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Tab {
    System,
    Containers,
    Images,
    Networks,
    Volumes,
}

impl AsRef<str> for Tab {
    fn as_ref(&self) -> &str {
        match &self {
            Tab::System => "System",
            Tab::Containers => "Containers",
            Tab::Images => "Images",
            Tab::Networks => "Networks",
            Tab::Volumes => "Volumes",
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug)]
pub struct Timers {
    pub update_time: SystemTime,
    pub data_usage: SystemTime,
    pub system_inspect: SystemTime,
    pub events: SystemTime,
}

impl Timers {
    pub fn restart(&mut self) {
        *self = Self::default();
    }
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            update_time: SystemTime::UNIX_EPOCH,
            data_usage: SystemTime::UNIX_EPOCH,
            system_inspect: SystemTime::UNIX_EPOCH,
            events: SystemTime::UNIX_EPOCH,
        }
    }
}

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,

    current_window: egui::Rect,
    errors: VecDeque<(SystemTime, String)>,

    current_tab: Tab,

    notifications: VecDeque<(SystemTime, String)>,
    containers: ContainersTab,
    images: ImagesTab,
    networks: NetworksTab,
    volumes: VolumesTab,
    system: SystemTab,

    settings_window: SettingsWindow,
    popups: VecDeque<ui::ActionPopup>,
    timers: Timers,
}

impl eframe::App for App {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_settings();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.display(ctx);
        self.display_windows(ctx);
    }
}

impl App {
    pub fn display(&mut self, ctx: &egui::Context) {
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
        //self.settings_window.settings.fonts.update_ctx(ctx);

        self.top_panel(ctx);
        self.side_panel(ctx);
        self.central_panel(ctx);
    }

    fn display_windows(&mut self, ctx: &egui::Context) {
        self.settings_window.display(ctx);
    }

    fn top_panel(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *ui::color::D_BG_00
            } else {
                *ui::color::L_BG_0
            },
            inner_margin: Margin::symmetric(5., 5.),
            ..Default::default()
        };
        egui::TopBottomPanel::top("top_panel")
            .frame(frame)
            .show(ctx, |ui| {
                let tabs = [
                    Tab::System,
                    Tab::Containers,
                    Tab::Images,
                    Tab::Networks,
                    Tab::Volumes,
                ];

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

    fn side_panel(&mut self, ctx: &egui::Context) {
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
                Tab::Networks => self.networks_side(ui),
                Tab::Volumes => self.volumes_side(ui),
            });
    }

    fn central_panel(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *ui::color::D_BG_0
            } else {
                *ui::color::L_BG_3
            },
            inner_margin: Margin::symmetric(10., 10.),
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
                Tab::Networks => {
                    self.networks_view(ui);
                }
                Tab::Volumes => {
                    self.volumes_view(ui);
                }
            }

            self.display_popups(ui);
        });
    }

    fn display_notifications_and_errors(&mut self, ctx: &egui::Context) {
        let mut offset = 0.;
        for (_, notification) in &self.notifications {
            if let Some(response) = egui::Window::new("Notification")
                .id(egui::Id::new(offset as u32))
                .anchor(egui::Align2::RIGHT_TOP, (0., offset))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(notification);
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

    fn display_popups(&mut self, ui: &mut egui::Ui) {
        use egui::Widget;
        for popup in &mut self.popups {
            popup.ui(ui);
            popup.ui(ui);
        }
    }
}

impl App {
    pub fn new(
        settings: Settings,
        tx_req: mpsc::Sender<EventRequest>,
        rx_rsp: mpsc::Receiver<EventResponse>,
    ) -> Self {
        let mut app = Self {
            tx_req,
            rx_rsp,

            current_tab: Tab::default(),
            current_window: egui::Rect::EVERYTHING,

            errors: VecDeque::new(),
            notifications: VecDeque::new(),
            containers: ContainersTab::default(),
            images: ImagesTab::default(),
            networks: NetworksTab::default(),
            volumes: VolumesTab::default(),
            system: SystemTab::default(),

            settings_window: SettingsWindow {
                settings,
                ..Default::default()
            },
            popups: VecDeque::new(),
            timers: Timers::default(),
        };
        app.send_event_notify(EventRequest::Container(ContainerEvent::List(Some(
            ContainerListOpts::builder().all(true).build(),
        ))));
        app.send_event_notify(EventRequest::Image(ImageEvent::List(None)));
        app.send_event_notify(EventRequest::Volume(VolumeEvent::List(None)));
        app
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
        let elapsed = self
            .timers
            .update_time
            .elapsed()
            .unwrap_or_default()
            .as_millis();

        if self
            .timers
            .system_inspect
            .elapsed()
            .unwrap_or_default()
            .as_secs()
            > 30
        {
            self.send_event_notify(EventRequest::SystemInspect);
            self.timers.system_inspect = SystemTime::now();
        }

        if self.timers.events.elapsed().unwrap_or_default().as_secs() > 10 {
            self.send_event_notify(EventRequest::SystemEvents);
            self.timers.events = SystemTime::now();
        }

        if self
            .timers
            .data_usage
            .elapsed()
            .unwrap_or_default()
            .as_secs()
            > 30
        {
            self.send_event_notify(EventRequest::SystemDataUsage);
            self.timers.data_usage = SystemTime::now();
        }

        match self.current_tab {
            Tab::Containers if elapsed > 1000 => {
                self.send_event_notify(EventRequest::Container(ContainerEvent::List(Some(
                    ContainerListOpts::builder().all(true).build(),
                ))));
                if self.containers.current_container.is_some() {
                    self.send_event_notify(EventRequest::Container(ContainerEvent::Details));
                    self.send_event_notify(EventRequest::Container(ContainerEvent::Logs));
                    if self
                        .containers
                        .current_container
                        .as_ref()
                        .map(|c| containers::is_running(c))
                        .unwrap_or_default()
                    {
                        self.send_event_notify(EventRequest::Container(
                            ContainerEvent::ProcessList,
                        ));
                        self.send_event_notify(EventRequest::Container(ContainerEvent::Stats));
                    }
                }
                self.timers.update_time = SystemTime::now();
            }
            Tab::Images if elapsed > 1000 => {
                self.send_event_notify(EventRequest::Image(ImageEvent::List(None)));
                if self.images.pull_view_data.in_progress
                    || self.images.search_view_data.pull_in_progress
                {
                    self.send_event_notify(EventRequest::Image(ImageEvent::PullChunks));
                }
                let id = self
                    .images
                    .current_image
                    .as_ref()
                    .map(|i| i.details.id.to_string());
                if let Some(id) = id {
                    self.send_event_notify(EventRequest::Image(ImageEvent::Inspect { id }));
                }
                self.timers.update_time = SystemTime::now();
            }
            Tab::Networks if elapsed > 1000 => {
                self.send_event_notify(EventRequest::Network(NetworkEvent::List(None)));
                self.timers.update_time = SystemTime::now();
            }
            Tab::Volumes if elapsed > 1000 => {
                self.send_event_notify(EventRequest::Volume(VolumeEvent::List(None)));
                self.timers.update_time = SystemTime::now();
            }
            _ => {}
        }
    }

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            //log::warn!("[gui] received event: {:?}", event);
            match event {
                EventResponse::Container(event) => self.handle_container_event_response(event),
                EventResponse::Image(event) => self.handle_image_event_response(event),
                EventResponse::Network(event) => self.handle_network_event_response(event),
                EventResponse::Volume(event) => self.handle_volume_event_response(event),
                EventResponse::DockerUriChange(res) => match res {
                    Ok(()) => {
                        self.clear_all();
                        self.add_notification("Successfully changed Docker uri")
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
                EventResponse::SystemEvents(events) => {
                    self.system.events_view_data.events.extend(events);
                }
                EventResponse::NotifyGui(event) => match event {
                    GuiEventResponse::SetTab(tab) => {
                        self.current_tab = tab;
                        match tab {
                            Tab::Containers => {
                                self.containers.central_view = containers::CentralView::Container;
                                self.containers.container_view = containers::ContainerView::Details;
                            }
                            Tab::Images => {
                                self.images.central_view = images::CentralView::Image;
                            }
                            _ => {}
                        }
                    }
                },
            }
        }
    }

    fn handle_container_event_response(&mut self, event: ContainerEventResponse) {
        use ContainerEventResponse::*;
        match event {
            List(mut containers) => {
                containers.sort_by(|a, b| match b.created.cmp(&a.created) {
                    std::cmp::Ordering::Equal => a.id.cmp(&b.id),
                    cmp => cmp,
                });
                self.containers.containers = containers
            }
            Details(container) => {
                self.send_event_notify(EventRequest::Container(ContainerEvent::Changes));
                self.set_container(container)
            }
            InspectNotFound => {
                self.add_error("container not found");
                self.containers.clear_container()
            }
            Delete(res) => match res {
                Ok(id) => self.add_notification(format!("successfully deleted container {}", id)),
                Err((id, e)) => match e {
                    docker_api::Error::Fault { code, message } => {
                        if code.as_u16() == 409 {
                            self.popups.push_back(ui::ActionPopup::builder(
                                    EventRequest::Container(ContainerEvent::ForceDelete { id })).title(
                                    "Force delete container").text(
                                    format!("{}\nAre you sure you want to forcefully delete this container?", message),
                                ).build());
                        } else {
                            self.add_error(format!(
                                "cannot force delete container {}: {}",
                                id, message
                            ));
                        }
                    }
                    _ => self.add_error(e),
                },
            },
            ForceDelete(res) => match res {
                Ok(id) => self.add_notification(format!("successfully deleted container {}", id)),
                Err(e) => self.add_error(e),
            },
            Stats(new_stats) => {
                if let Some(stats) = &mut self.containers.current_stats {
                    stats.extend(*new_stats);
                } else {
                    self.containers.current_stats = Some(new_stats)
                }
            }
            Logs(logs) => {
                let raw_bytes = logs
                    .0
                    .clone()
                    .into_iter()
                    .flat_map(|x| match x {
                        TtyChunk::StdOut(data) => data,
                        TtyChunk::StdErr(data) => data,
                        TtyChunk::StdIn(_) => unreachable!(),
                    })
                    .collect::<Vec<_>>();
                let escaped_bytes = strip_ansi_escapes::strip(&raw_bytes).unwrap_or(raw_bytes);
                let logs = String::from_utf8_lossy(&escaped_bytes);
                if let Some(current_logs) = &mut self.containers.logs_view_data.current_logs {
                    current_logs.push_str(&logs);
                } else {
                    self.containers.logs_view_data.current_logs = Some(logs.to_string());
                }
            }
            Start(res) | Stop(res) | Pause(res) | Unpause(res) => {
                if let Err(e) = res {
                    self.add_error(e);
                }
            }
            Create(res) => match res {
                Ok(id) => self.add_notification(format!("successfully created container {}", id)),
                Err(e) => self.add_error(e),
            },
            Rename(res) => match res {
                Ok(_) => self.add_notification("successfully renamed the container"),
                Err(e) => self.add_error(e),
            },
            Prune(res) => match res {
                Ok(info) => {
                    if let Some(deleted) = info.containers_deleted {
                        let deleted =
                            deleted
                                .into_iter()
                                .fold("Deleted:\n".to_string(), |mut acc, c| {
                                    acc.push_str(" - ");
                                    acc.push_str(&c);
                                    acc.push('\n');
                                    acc
                                });
                        self.add_notification(format!(
                            "Space reclaimed: {}\n\n{}",
                            crate::conv_b(info.space_reclaimed as u64),
                            deleted
                        ));
                        self.send_event_notify(EventRequest::SystemDataUsage);
                    }
                }
                Err(e) => self.add_error(e),
            },
            Restart(res) => match res {
                Ok(id) => {
                    self.add_notification(format!("successfully restarted the container `{}`", id))
                }
                Err(e) => self.add_error(e),
            },
            RestartInProgress { id } => {
                self.add_notification(format!("container `{}` restart in progress", id))
            }
            ProcessList(res) => match res {
                Ok(top) => self.containers.current_top = Some(top),
                Err(e) => self.add_error(e),
            },
            Changes(res) => match res {
                Ok(mut changes) => {
                    changes.as_deref_mut().map(|c| {
                        c.sort_by(|a, b| match a.kind.partial_cmp(&b.kind) {
                            Some(std::cmp::Ordering::Equal) | None => a.path.cmp(&b.path),
                            Some(cmp) => cmp,
                        });
                        c
                    });
                    self.containers.changes_view_data.current_changes = changes
                }
                Err(e) => self.add_error(e),
            },
        }
    }

    fn handle_image_event_response(&mut self, event: ImageEventResponse) {
        use ImageEventResponse::*;
        match event {
            List(mut images) => {
                images.sort_by(|a, b| match b.created.cmp(&a.created) {
                    std::cmp::Ordering::Equal => a.id.cmp(&b.id),
                    cmp => cmp,
                });
                self.images.images = images
            }
            Inspect(image) => self.images.current_image = Some(image),
            Delete(res) => {
                match res {
                    Ok(status) => {
                        let status = format_status(status);
                        self.add_notification(status)
                    }
                    Err((id, e)) => match e {
                        docker_api::Error::Fault { code, message } => {
                            if code.as_u16() == 409 && !message.contains("cannot be forced") {
                                self.popups.push_back(ui::ActionPopup::builder(
                                    EventRequest::Image(ImageEvent::ForceDelete { id })).title(
                                    "Force delete image").text(
                                    format!("{}\n Are you sure you want to forcefully delete this image?", message),
                                ).build());
                            } else {
                                self.add_error(format!(
                                    "cannot force delete image {}: {}",
                                    id, message
                                ));
                            }
                        }
                        _ => self.add_error(e),
                    },
                }
            }
            ForceDelete(res) => match res {
                Ok(status) => {
                    let status = format_status(status);
                    self.add_notification(status);
                }
                Err(e) => self.add_error(e),
            },
            Save(res) => match res {
                Ok((id, path)) => self.add_notification(format!(
                    "successfully exported image {} to tar archive in `{}`",
                    id,
                    path.display()
                )),
                Err(e) => self.add_error(e),
            },
            Pull(res) => match res {
                Ok(id) => {
                    self.images.pull_view_data.in_progress = false;
                    self.add_notification(format!("successfully pulled image {}", id,))
                }
                Err(e) => self.add_error(e),
            },
            PullChunks(new_chunks) => {
                if let Some(chunks) = &mut self.images.current_pull_chunks {
                    chunks.extend(new_chunks);
                } else {
                    self.images.current_pull_chunks = Some(new_chunks);
                }
            }
            Search(res) => match res {
                Ok(mut results) => {
                    results.sort_by(|a, b| b.star_count.cmp(&a.star_count));
                    self.images.search_view_data.images = Some(results)
                }
                Err(e) => self.add_error(e),
            },
            Import(res) => match res {
                Ok(path) => {
                    self.add_notification(format!("successfully imported image `{}`", path))
                }
                Err(e) => self.add_error(e),
            },
            Prune(res) => match res {
                Ok(info) => {
                    let (untagged, deleted) = info
                        .images_deleted
                        .map(|images| {
                            images.into_iter().fold(
                                ("Untagged:\n".to_string(), "Deleted:\n".to_string()),
                                |(mut untagged, mut deleted), i| {
                                    if let Some(u) = i.untagged {
                                        untagged.push_str(" - ");
                                        untagged.push_str(&u);
                                        untagged.push('\n');
                                    }
                                    if let Some(d) = i.deleted {
                                        deleted.push_str(" - ");
                                        deleted.push_str(&d);
                                        deleted.push('\n');
                                    }
                                    (untagged, deleted)
                                },
                            )
                        })
                        .unwrap_or_default();
                    self.add_notification(format!(
                        "Space reclaimed: {}\n\n{}\n{}",
                        crate::conv_b(info.space_reclaimed as u64),
                        untagged,
                        deleted
                    ));
                    self.send_event_notify(EventRequest::SystemDataUsage);
                }
                Err(e) => self.add_error(e),
            },
            ClearCache(res) => match res {
                Ok(info) => {
                    let deleted = info
                        .caches_deleted
                        .map(|caches| {
                            caches
                                .into_iter()
                                .fold("Deleted:\n".to_string(), |mut acc, c| {
                                    acc.push_str(" - ");
                                    acc.push_str(&c);
                                    acc.push('\n');
                                    acc
                                })
                        })
                        .unwrap_or_default();
                    self.add_notification(format!(
                        "Space reclaimed: {}\n\n{}",
                        crate::conv_b(info.space_reclaimed as u64),
                        deleted
                    ));
                    self.send_event_notify(EventRequest::SystemDataUsage);
                }
                Err(e) => self.add_error(e),
            },
            Tag(res) => match res {
                Ok(_) => self.add_notification("successfully tagged image"),
                Err(e) => self.add_error(e),
            },
        }
    }

    fn handle_network_event_response(&mut self, event: NetworkEventResponse) {
        use NetworkEventResponse::*;
        match event {
            Delete(res) => match res {
                Ok(id) => self.add_notification(format!("successfully deleted network {}", id)),
                Err(e) => self.add_error(e),
            },
            List(mut networks) => {
                networks.sort_by(|a, b| match b.created.cmp(&a.created) {
                    std::cmp::Ordering::Equal => a.id.cmp(&b.id),
                    cmp => cmp,
                });
                self.networks.networks = networks;
            }
            Prune(res) => match res {
                Ok(info) => {
                    let status = info.networks_deleted.into_iter().fold(
                        "Successfully deleted networks:\n".to_string(),
                        |mut acc, n| {
                            acc.push_str(" - ");
                            acc.push_str(&n);
                            acc.push('\n');
                            acc
                        },
                    );
                    self.add_notification(status)
                }
                Err(e) => self.add_error(e),
            },
            Create(res) => match res {
                Ok(id) => self.add_notification(format!("successfully created network {}", id)),
                Err(e) => self.add_error(e),
            },
        }
    }

    fn handle_volume_event_response(&mut self, event: VolumeEventResponse) {
        use VolumeEventResponse::*;
        match event {
            Delete(res) => match res {
                Ok(id) => self.add_notification(format!("successfully deleted volume {}", id)),
                Err(e) => self.add_error(e),
            },
            List(res) => match res {
                Ok(mut volumes) => {
                    volumes
                        .volumes
                        .sort_by(|a, b| match b.created_at.cmp(&a.created_at) {
                            std::cmp::Ordering::Equal => a.name.cmp(&b.name),
                            cmp => cmp,
                        });
                    self.volumes.volumes = Some(volumes);
                }
                Err(e) => self.add_error(e),
            },
            Prune(res) => match res {
                Ok(info) => {
                    let status = info.volumes_deleted.into_iter().fold(
                        format!(
                            "Space reclaimed: {}\n\nVolumes deleted:\n",
                            crate::conv_b(info.space_reclaimed as u64)
                        ),
                        |mut acc, n| {
                            acc.push_str(" - ");
                            acc.push_str(&n);
                            acc.push('\n');
                            acc
                        },
                    );
                    self.add_notification(status)
                }
                Err(e) => self.add_error(e),
            },
            Create(res) => match res {
                Ok(name) => self.add_notification(format!("successfully created volume {}", name)),
                Err(e) => self.add_error(e),
            },
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
        self.timers.restart();
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
            self.send_event_notify(EventRequest::Container(ContainerEvent::TraceStart {
                id: container.id.clone(),
            }));
        }

        self.containers.current_container = Some(container);
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings_window.save_settings() {
            self.add_error(e);
        } else {
            self.send_event_notify(EventRequest::DockerUriChange {
                uri: self.docker_uri(),
            });
        }
    }

    pub fn docker_uri(&self) -> String {
        if self.settings_window.settings.use_docker_host_env {
            match std::env::var("DOCKER_HOST") {
                Ok(uri) => return uri,
                Err(e) => {
                    log::error!("failed to read DOCKER_HOST environment variable: {}", e);
                }
            }
        }
        self.settings_window.settings.docker_addr.clone()
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

fn format_status(status: Vec<docker_api::api::Status>) -> String {
    status.into_iter().fold(String::new(), |mut acc, s| {
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
    })
}
