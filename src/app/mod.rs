mod containers;
mod images;

use crate::event::{EventRequest, EventResponse};
use containers::ContainersTab;
use images::ImagesTab;

use anyhow::{Context, Result};
use docker_api::api::{ContainerDetails, ContainerListOpts, Status};
use eframe::{egui, epi};
use std::collections::VecDeque;
use std::time::SystemTime;
use tokio::sync::mpsc;

mod colors {
    use egui::{
        style::{Selection, Widgets},
        Color32, Rgba, Stroke, Visuals,
    };
    use epaint::Shadow;
    use lazy_static::lazy_static;

    lazy_static! {
        pub static ref D_BG_000: Color32 = Color32::from_rgb(0x0e, 0x12, 0x17);
        pub static ref D_BG_00: Color32 = Color32::from_rgb(0x11, 0x16, 0x1b);
        pub static ref D_BG_0: Color32 = Color32::from_rgb(0x16, 0x1c, 0x23);
        pub static ref D_BG_1: Color32 = Color32::from_rgb(0x23, 0x2d, 0x38);
        pub static ref D_BG_2: Color32 = Color32::from_rgb(0x31, 0x3f, 0x4e);
        pub static ref D_BG_3: Color32 = Color32::from_rgb(0x41, 0x53, 0x67);
        pub static ref D_FG_0: Color32 = Color32::from_rgb(0xe5, 0xde, 0xd6);
        pub static ref D_BG_00_TRANSPARENT: Color32 = Rgba::from(*D_BG_00).multiply(0.96).into();
        pub static ref D_BG_0_TRANSPARENT: Color32 = Rgba::from(*D_BG_0).multiply(0.96).into();
        pub static ref D_BG_1_TRANSPARENT: Color32 = Rgba::from(*D_BG_1).multiply(0.96).into();
        pub static ref D_BG_2_TRANSPARENT: Color32 = Rgba::from(*D_BG_2).multiply(0.96).into();
        pub static ref D_BG_3_TRANSPARENT: Color32 = Rgba::from(*D_BG_3).multiply(0.96).into();
        pub static ref L_BG_0: Color32 = Color32::from_rgb(0xbf, 0xbf, 0xbf);
        pub static ref L_BG_1: Color32 = Color32::from_rgb(0xd4, 0xd3, 0xd4);
        pub static ref L_BG_2: Color32 = Color32::from_rgb(0xd9, 0xd9, 0xd9);
        pub static ref L_BG_3: Color32 = Color32::from_rgb(0xea, 0xea, 0xea);
        pub static ref L_BG_4: Color32 = Color32::from_rgb(0xf9, 0xf9, 0xf9);
        pub static ref L_BG_5: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
        pub static ref L_BG_0_TRANSPARENT: Color32 = Rgba::from(*L_BG_0).multiply(0.86).into();
        pub static ref L_BG_1_TRANSPARENT: Color32 = Rgba::from(*L_BG_1).multiply(0.86).into();
        pub static ref L_BG_2_TRANSPARENT: Color32 = Rgba::from(*L_BG_2).multiply(0.86).into();
        pub static ref L_BG_3_TRANSPARENT: Color32 = Rgba::from(*L_BG_3).multiply(0.86).into();
        pub static ref L_BG_4_TRANSPARENT: Color32 = Rgba::from(*L_BG_4).multiply(0.86).into();
        pub static ref L_BG_5_TRANSPARENT: Color32 = Rgba::from(*L_BG_5).multiply(0.86).into();
        pub static ref L_FG_0: Color32 = *D_BG_0;
    }

    pub fn light_visuals() -> Visuals {
        let mut widgets = Widgets::light();
        widgets.noninteractive.bg_fill = *L_BG_3_TRANSPARENT;
        widgets.inactive.bg_fill = *L_BG_3_TRANSPARENT;
        widgets.inactive.bg_stroke = Stroke::new(0.5, *D_BG_3);
        widgets.inactive.fg_stroke = Stroke::new(0.5, *D_BG_3);
        widgets.hovered.bg_fill = *L_BG_4_TRANSPARENT;
        widgets.hovered.bg_stroke = Stroke::new(1., *D_BG_1);
        widgets.hovered.fg_stroke = Stroke::new(1., *D_BG_1);
        widgets.active.bg_fill = *L_BG_5_TRANSPARENT;
        widgets.active.fg_stroke = Stroke::new(1.5, *D_BG_0);
        widgets.active.bg_stroke = Stroke::new(1.5, *D_BG_0);

        Visuals {
            dark_mode: false,
            extreme_bg_color: Color32::WHITE,
            selection: Selection {
                bg_fill: *L_BG_5,
                stroke: Stroke::new(0.7, *D_BG_0),
            },
            popup_shadow: Shadow::small_light(),
            widgets,
            ..Default::default()
        }
    }

    pub fn dark_visuals() -> Visuals {
        let mut widgets = Widgets::dark();
        widgets.noninteractive.bg_fill = *D_BG_0_TRANSPARENT;
        widgets.inactive.bg_fill = *D_BG_1_TRANSPARENT;
        widgets.hovered.bg_fill = *D_BG_2_TRANSPARENT;
        widgets.active.bg_fill = *D_BG_0_TRANSPARENT;

        Visuals {
            dark_mode: true,
            extreme_bg_color: Color32::BLACK,
            selection: Selection {
                bg_fill: *D_BG_3_TRANSPARENT,
                stroke: Stroke::new(0.7, *D_FG_0),
            },
            popup_shadow: Shadow::small_dark(),
            widgets,
            ..Default::default()
        }
    }
}

const PACKAGE_ICON: &str = "\u{1F4E6}";
const SCROLL_ICON: &str = "\u{1F4DC}";
const INFO_ICON: &str = "\u{2139}";
const DELETE_ICON: &str = "\u{1F5D9}";
const PLAY_ICON: &str = "\u{25B6}";
const PAUSE_ICON: &str = "\u{23F8}";
const STOP_ICON: &str = "\u{23F9}";
const SETTINGS_ICON: &str = "\u{2699}";
const SAVE_ICON: &str = "\u{1F4BE}";

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
    current_window: egui::Rect,
    errors: VecDeque<(SystemTime, String)>,

    current_tab: Tab,

    notifications: VecDeque<(SystemTime, String)>,
    containers: ContainersTab,
    images: ImagesTab,

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
        if ctx.style().visuals.dark_mode {
            ctx.set_visuals(colors::dark_visuals());
        } else {
            ctx.set_visuals(colors::light_visuals());
        }
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
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *colors::D_BG_00
            } else {
                *colors::L_BG_0
            },
            margin: egui::vec2(5., 5.),
            ..Default::default()
        };
        egui::TopBottomPanel::top("top_panel")
            .frame(frame)
            .show(ctx, |ui| {
                let tabs = [Tab::Containers, Tab::Images];

                ui.horizontal(|ui| {
                    egui::Grid::new("tab_grid").show(ui, |ui| {
                        for tab in tabs {
                            ui.selectable_value(&mut self.current_tab, tab, tab.as_ref());
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(), |ui| {
                        egui::global_dark_light_mode_switch(ui);

                        if ui.button(SETTINGS_ICON).clicked() {
                            self.settings_window.toggle();
                        }
                    });
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
        let frame = egui::Frame {
            fill: if ctx.style().visuals.dark_mode {
                *colors::D_BG_00
            } else {
                *colors::L_BG_0
            },
            ..Default::default()
        };
        egui::SidePanel::left("side_panel")
            .frame(frame)
            .min_width(100.)
            .max_width(250.)
            .max_width(self.side_panel_size())
            .resizable(false)
            .show(ctx, |ui| match self.current_tab {
                Tab::Containers => {
                    self.containers_scroll(ui);
                }
                Tab::Images => {
                    self.image_side(ui);
                }
            });
    }

    fn central_panel(&mut self, ctx: &egui::CtxRef) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.display_notifications_and_errors(ctx);
            match self.current_tab {
                Tab::Containers => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.container_details(ui));
                }
                Tab::Images => {
                    egui::ScrollArea::vertical().show(ui, |ui| self.image_view(ui));
                }
            }
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
}

impl App {
    pub fn new(tx_req: mpsc::Sender<EventRequest>, rx_rsp: mpsc::Receiver<EventResponse>) -> Self {
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
        self.notifications
            .push_back((SystemTime::now(), format!("{}", notification)));
    }

    fn add_error(&mut self, error: impl std::fmt::Debug) {
        self.errors
            .push_back((SystemTime::now(), format!("{:?}", error)));
    }

    fn send_update_request(&mut self) {
        self.send_event_notify(EventRequest::ListContainers(Some(
            ContainerListOpts::builder().all(true).build(),
        )));
        self.send_event_notify(EventRequest::ListImages(None));
        if self.images.pull_view.in_progress {
            self.send_event_notify(EventRequest::PullImageChunks);
        }
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

    fn read_worker_events(&mut self) {
        while let Ok(event) = self.rx_rsp.try_recv() {
            //log::warn!("[gui] received event: {:?}", event);
            match event {
                EventResponse::ListContainers(containers) => {
                    self.containers.containers = containers
                }
                EventResponse::ListImages(images) => self.images.images = images,
                EventResponse::ContainerDetails(container) => self.set_container(container),
                EventResponse::InspectContainerNotFound => {
                    self.add_error("container not found");
                    self.clear_container()
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

    fn handle_data_update(&mut self) {
        if self.update_time.elapsed().unwrap_or_default().as_millis() > 1000 {
            self.send_update_request();
        }
    }

    fn clear_container(&mut self) {
        self.containers.current_container = None;
        self.containers.current_stats = None;
        self.containers.current_logs = None;
        self.containers.logs_page = 0;
    }

    fn set_container(&mut self, container: Box<ContainerDetails>) {
        let changed = self
            .containers
            .current_container
            .as_ref()
            .map(|current| current.id != container.id)
            .unwrap_or(true);

        if changed {
            self.clear_container();
            if let Err(e) = self.send_event(EventRequest::ContainerTraceStart {
                id: container.id.clone(),
            }) {
                self.add_error(e);
            }
        }

        self.containers.current_container = Some(container);
    }
}

fn line(ui: &mut egui::Ui, frame: egui::Frame) -> egui::Response {
    frame
        .show(ui, |ui| {
            ui.set_max_height(1.);
            let available_space = ui.available_size();

            let size = egui::vec2(available_space.x, 0.);

            let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());
            let points = [
                egui::pos2(rect.left(), rect.bottom()),
                egui::pos2(rect.right(), rect.bottom()),
            ];

            let stroke = ui.visuals().widgets.noninteractive.fg_stroke;
            ui.painter().line_segment(points, stroke);
            response
        })
        .response
}
