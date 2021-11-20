use crate::{EventRequest, EventResponse, StatsWrapper};
use anyhow::{Context, Result};
use docker_api::api::{
    ContainerDetails, ContainerInfo, ContainerListOpts, ContainerStatus, ImageInfo,
};
use eframe::{egui, epi};
use egui::widgets::plot::{self, Line, Plot};
use egui::Label;
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;

const DOT: &str = "\u{25CF}";
const INFO_ICON: &str = "\u{2139}";
const DELETE_ICON: &str = "\u{1F5D9}";

pub struct App {
    tx_req: mpsc::Sender<EventRequest>,
    rx_rsp: mpsc::Receiver<EventResponse>,
    containers: Vec<ContainerInfo>,
    images: Vec<ImageInfo>,
    update_time: SystemTime,
    notifications_time: SystemTime,

    current_container: Option<ContainerDetails>,
    current_stats: Option<Vec<(Duration, StatsWrapper)>>,
    notifications: VecDeque<String>,
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
    fn top_panel(&self, ctx: &egui::CtxRef) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |_| {});
    }

    fn side_panel(&mut self, ctx: &egui::CtxRef) {
        egui::SidePanel::left("side_panel")
            .min_width(150.)
            .show(ctx, |ui| {
                self.containers_scroll(ui);
            });
    }

    fn central_panel(&mut self, ctx: &egui::CtxRef) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.display_notifications(ctx);
            egui::ScrollArea::vertical()
                .always_show_scroll(true)
                .show(ui, |ui| self.container_details(ui));
        });
    }

    fn containers_scroll(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("side_panel").show(ui, |ui| {
                let mut errors = vec![];
                for container in &self.containers {
                    let color = if &container.state == "running" {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::RED
                    };
                    let dot = egui::Label::new(DOT).text_color(color);
                    ui.scope(|ui| {
                        ui.add(dot);
                        if let Some(name) = container.names.first() {
                            ui.add(Label::new(name.trim_start_matches('/')).strong());
                        } else {
                            ui.add(Label::new(&container.id[..12]).strong());
                        }
                    });

                    ui.scope(|ui| {
                        if ui.button(INFO_ICON).clicked() {
                            if let Err(e) = self.send_event(EventRequest::InspectContainer {
                                id: container.id.clone(),
                            }) {
                                errors.push(e);
                            };
                        }
                        if ui.button(DELETE_ICON).clicked() {
                            if let Err(e) = self.send_event(EventRequest::DeleteContainer {
                                id: container.id.clone(),
                            }) {
                                errors.push(e);
                            };
                        }
                    });
                    ui.end_row();

                    let image = if container.image.starts_with("sha256") {
                        &container.image.trim_start_matches("sha256:")[..12]
                    } else {
                        container.image.as_str()
                    };
                    ui.add(Label::new(image).italics());
                    ui.end_row();

                    ui.add(Label::new(&container.status).italics());
                    ui.end_row();
                }
                errors.iter().for_each(|err| self.add_notification(err));
            });
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

    fn container_details(&mut self, ui: &mut egui::Ui) {
        if let Some(container) = &self.current_container {
            ui.heading(&container.id);
            ui.horizontal(|ui| if ui.button("stop").clicked() {});
            egui::Grid::new("container_details").show(ui, |ui| {
                macro_rules! key {
                    ($k:literal) => {
                        ui.add(Label::new($k).strong());
                    };
                }
                macro_rules! val {
                    ($v:expr) => {
                        ui.add(Label::new($v).monospace());
                    };
                }
                key!("Image: ");
                val!(&container.image);
                ui.end_row();

                key!("Command: ");
                val!(&container.config.cmd.as_deref().unwrap_or(&[]).join(" "));
                ui.end_row();

                if let Some(entrypoint) = container.config.entrypoint.as_ref() {
                    key!("Entrypoint: ");
                    val!(&entrypoint.join(" "));
                    ui.end_row();
                }

                key!("Labels: ");
                ui.end_row();
                if let Some(labels) = container.config.labels.as_ref() {
                    if !labels.is_empty() {
                        ui.label("          ");
                        egui::Grid::new("labels_grid").show(ui, |ui| {
                            for (k, v) in labels {
                                ui.add(Label::new(&k).monospace());
                                ui.add(Label::new(&v).monospace());
                                ui.end_row();
                            }
                        });
                        ui.end_row();
                    }
                }

                key!("Created: ");
                val!(container.created.to_rfc2822());
                ui.end_row();

                key!("State: ");
                val!(container.state.status.as_ref());
                ui.end_row();

                key!("Hostname: ");
                val!(&container.config.hostname);
                ui.end_row();

                if !container.config.domainname.is_empty() {
                    key!("Domainname: ");
                    val!(&container.config.domainname);
                    ui.end_row();
                }

                key!("User: ");
                val!(&container.config.user);
                ui.end_row();

                key!("Working dir: ");
                val!(&container.config.working_dir);
                ui.end_row();

                if let Some(shell) = container.config.shell.as_ref() {
                    key!("Shell: ");
                    val!(&shell.join(" "));
                    ui.end_row();
                }

                key!("Env: ");
                ui.end_row();
                if !container.config.env.is_empty() {
                    for var in &container.config.env {
                        ui.scope(|_| {});
                        val!(&var);
                        ui.end_row();
                    }
                }

                if let Some(size_rw) = &container.size_rw {
                    key!("Size RW: ");
                    val!(&format!("{}", size_rw));
                    ui.end_row();
                }

                if let Some(size_root_fs) = &container.size_root_fs {
                    key!("Size root FS: ");
                    val!(&format!("{}", size_root_fs));
                    ui.end_row();
                }

                key!("Restart count: ");
                val!(&format!("{}", container.restart_count));
                ui.end_row();

                key!("Driver: ");
                val!(&container.driver);
                ui.end_row();

                key!("Platform: ");
                val!(&container.platform);
                ui.end_row();

                key!("Networks:");
                ui.end_row();
                if !container.network_settings.networks.is_empty() {
                    ui.scope(|_| {});
                    egui::Grid::new("networks_grid").show(ui, |ui| {
                        for (name, entry) in &container.network_settings.networks {
                            ui.add(Label::new(name).strong());
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("MAC address:").strong());
                            ui.add(Label::new(&entry.gateway).monospace());
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("IPv4:").strong());
                            ui.add(
                                Label::new(format!("{}/{}", entry.ip_address, entry.ip_prefix_len))
                                    .monospace(),
                            );
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("Gateway:").strong());
                            ui.add(Label::new(&entry.gateway).monospace());
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("IPv6:").strong());
                            ui.add(
                                Label::new(format!(
                                    "{}/{}",
                                    entry.global_ipv6_address, entry.global_ipv6_prefix_len
                                ))
                                .monospace(),
                            );
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("IPv6 Gateway:").strong());
                            ui.add(Label::new(&entry.ipv6_gateway).monospace());
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("Network ID:").strong());
                            ui.add(Label::new(&entry.network_id).monospace());
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.add(Label::new("Endpoint ID:").strong());
                            ui.add(Label::new(&entry.endpoint_id).monospace());
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

            if let Some(stats) = &self.current_stats {
                let cpu_data = plot::Values::from_values_iter(
                    stats
                        .iter()
                        .map(|(time, stat)| plot::Value::new(time.as_secs_f64(), stat.cpu_usage)),
                );

                ui.add(
                    Plot::new("cpu_usage")
                        .height(500.)
                        .include_x(0.)
                        .include_y(0.)
                        .line(Line::new(cpu_data)),
                );
            }
        }
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
            notifications_time: SystemTime::now(),
            current_container: None,
            current_stats: None,
            notifications: VecDeque::new(),
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
        if self
            .current_container
            .as_ref()
            .map(|c| match c.state.status {
                ContainerStatus::Running
                | ContainerStatus::Created
                | ContainerStatus::Restarting => true,
                _ => false,
            })
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
                EventResponse::InspectContainer(container) => {
                    if let Some(current) = &self.current_container {
                        if &current.id != &container.id {
                            let _ = self.send_event(EventRequest::ContainerStatsStart {
                                id: container.id.clone(),
                            });
                        }
                    } else {
                        let _ = self.send_event(EventRequest::ContainerStatsStart {
                            id: container.id.clone(),
                        });
                    }
                    self.current_container = Some(container)
                }
                EventResponse::DeleteContainer(msg) => self.notifications.push_back(msg),
                EventResponse::ContainerStats(stats) => self.current_stats = Some(stats),
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
}
