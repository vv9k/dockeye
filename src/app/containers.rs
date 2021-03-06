use crate::app::{
    ui,
    ui::icon,
    ui::{color, key, key_val, val},
    App,
};
use crate::event::{ContainerEvent, EventRequest, GuiEvent};
use crate::worker::RunningContainerStats;

use anyhow::{Context, Result};
use docker_api::api::{
    Change, ChangeKind, ContainerCreateOpts, ContainerDetails, ContainerId, ContainerIdRef,
    ContainerInfo, ContainerStatus, Top,
};
use egui::containers::Frame;
use egui::widgets::plot::{self, Line, Plot};
use egui::{style::Margin, Grid, Label, RichText};

const PAGE_SIZE: usize = 1024;

pub fn color_for_state(state: &ContainerStatus) -> egui::Color32 {
    match state {
        ContainerStatus::Running => egui::Color32::GREEN,
        ContainerStatus::Paused => egui::Color32::YELLOW,
        _ => egui::Color32::RED,
    }
}

pub fn state_icon(color: egui::Color32) -> Label {
    Label::new(RichText::new(icon::PACKAGE).color(color).heading().strong())
}

pub fn is_running(container: &ContainerDetails) -> bool {
    matches!(container.state.status, ContainerStatus::Running)
}

pub fn is_paused(container: &ContainerDetails) -> bool {
    matches!(container.state.status, ContainerStatus::Paused)
}
macro_rules! btn {
    ($self:ident, $ui:ident, $icon:expr, $hover:literal, $event:expr, $errors: expr) => {
        if $ui.button($icon).on_hover_text($hover).clicked() {
            if let Err(e) = $self.send_event($event) {
                $errors = Some(Box::new(e));
            }
        }
    };
    (stop => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::STOP,
            "stop this container",
            EventRequest::Container(ContainerEvent::Stop {
                id: $container.id.clone()
            }),
            $errors
        );
    };
    (start => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::PLAY,
            "start this container",
            EventRequest::Container(ContainerEvent::Start {
                id: $container.id.clone()
            }),
            $errors
        );
    };
    (pause => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::PAUSE,
            "pause this container",
            EventRequest::Container(ContainerEvent::Pause {
                id: $container.id.clone()
            }),
            $errors
        );
    };
    (unpause => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::PLAY,
            "unpause this container",
            EventRequest::Container(ContainerEvent::Unpause {
                id: $container.id.clone()
            }),
            $errors
        );
    };
    (info => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::INFO,
            "inpect this container",
            EventRequest::Container(ContainerEvent::TraceStart {
                id: $container.id.clone()
            }),
            $errors
        );
    };
    (restart => $self:ident, $ui:ident, $container:ident, $errors:expr) => {
        btn!(
            $self,
            $ui,
            icon::RESTART,
            "restart this container",
            EventRequest::Container(ContainerEvent::Restart {
                id: $container.id.clone()
            }),
            $errors
        );
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Decides which main view is displayed on the central panel
pub enum CentralView {
    None,
    Container,
    Create,
}

impl Default for CentralView {
    fn default() -> Self {
        CentralView::None
    }
}

#[derive(Debug, PartialEq)]
/// Decides which tab is open when displaying a detailed view of a container
pub enum ContainerView {
    Details,
    Logs,
    Processes,
    Changes,
    Attach,
}

impl Default for ContainerView {
    fn default() -> Self {
        ContainerView::Details
    }
}

#[derive(Debug)]
pub struct ContainerCreateData {
    pub image: String,
    pub command: String,
    pub name: String,
    pub working_dir: String,
    pub user: String,
    pub userns_mode: String,
    pub network_mode: String,
    pub log_driver: String,
    pub tty: bool,
    pub stdin: bool,
    pub stderr: bool,
    pub stdout: bool,
    pub privileged: bool,
    pub autoremove: bool,
    pub cpus: f32,
    pub cpu_shares: f32,
    pub memory: String,
    pub memory_swap: String,
    pub env: Vec<(String, String)>,
    pub labels: Vec<(String, String)>,
    pub sec_ops: Vec<String>,
    pub volumes: Vec<String>,
    pub links: Vec<String>,
    pub capabilities: Vec<String>,
}

impl Default for ContainerCreateData {
    fn default() -> Self {
        Self {
            image: "".to_string(),
            command: "".to_string(),
            name: "".to_string(),
            working_dir: "".to_string(),
            user: "".to_string(),
            userns_mode: "".to_string(),
            network_mode: "".to_string(),
            log_driver: "".to_string(),
            tty: false,
            stdin: false,
            stderr: false,
            stdout: false,
            privileged: false,
            autoremove: false,
            cpus: 0.,
            cpu_shares: -1.,
            memory: "".to_string(),
            memory_swap: "".to_string(),
            env: vec![],
            labels: vec![],
            sec_ops: vec![],
            volumes: vec![],
            links: vec![],
            capabilities: vec![],
        }
    }
}

impl ContainerCreateData {
    pub fn reset(&mut self) {
        *self = ContainerCreateData::default();
    }

    pub fn as_opts(&self) -> Result<ContainerCreateOpts> {
        let mut opts = ContainerCreateOpts::builder(&self.image);
        if !self.command.is_empty() {
            // #TODO: this should be wiser about arguments
            opts = opts.cmd(self.command.split_ascii_whitespace());
        }
        if !self.name.is_empty() {
            opts = opts.name(&self.name);
        }
        if !self.working_dir.is_empty() {
            opts = opts.working_dir(&self.working_dir);
        }
        if !self.user.is_empty() {
            opts = opts.user(&self.user);
        }
        if !self.userns_mode.is_empty() {
            opts = opts.userns_mode(&self.userns_mode);
        }
        if !self.network_mode.is_empty() {
            opts = opts.network_mode(&self.network_mode);
        }
        if !self.log_driver.is_empty() {
            opts = opts.log_driver(&self.log_driver);
        }
        opts = opts.tty(self.tty);
        opts = opts.attach_stdin(self.stdin);
        opts = opts.attach_stderr(self.stderr);
        opts = opts.attach_stdout(self.stdout);
        opts = opts.privileged(self.privileged);
        opts = opts.auto_remove(self.autoremove);
        opts = opts.cpus(self.cpus as f64);
        if self.cpu_shares > 0. {
            opts = opts.cpu_shares(self.cpu_shares as u32);
        }
        if !self.memory.is_empty() {
            match crate::convert_memory(&self.memory) {
                Ok(memory) => {
                    opts = opts.memory(memory);
                }
                Err(e) => return Err(e),
            }
        }
        if !self.memory_swap.is_empty() {
            match crate::convert_memory(&self.memory_swap) {
                Ok(swap) => match swap.try_into().context("converting memory swap failed") {
                    Ok(swap) => opts = opts.memory_swap(swap),
                    Err(e) => return Err(e),
                },
                Err(e) => return Err(e),
            }
        }

        if !self.env.is_empty() {
            let env = self
                .env
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>();
            opts = opts.env(env);
        }

        if !self.labels.is_empty() {
            opts = opts.labels(self.labels.clone());
        }
        if !self.sec_ops.is_empty() {
            opts = opts.security_options(self.sec_ops.clone());
        }
        if !self.volumes.is_empty() {
            opts = opts.volumes(self.volumes.clone());
        }
        if !self.links.is_empty() {
            opts = opts.links(self.links.clone());
        }
        if !self.capabilities.is_empty() {
            opts = opts.capabilities(self.capabilities.clone());
        }

        Ok(opts.build())
    }
}

#[derive(Debug, Default)]
pub struct RenameWindow {
    pub show: bool,
    pub id: ContainerId,
    pub new_name: String,
}

impl RenameWindow {
    fn toggle(&mut self) {
        self.show = !self.show;
    }
}

#[derive(Debug, Default)]
pub struct LogsViewData {
    pub current_logs: Option<String>,
    pub page: usize,
    pub follow: bool,
}

#[derive(Debug, Default)]
pub struct ChangesViewData {
    pub current_changes: Option<Vec<Change>>,
    pub page: usize,
}

#[derive(Debug, Default)]
pub struct ContainersTab {
    pub containers: Vec<ContainerInfo>,
    pub current_container: Option<Box<ContainerDetails>>,
    pub current_stats: Option<Box<RunningContainerStats>>,
    pub current_top: Option<Top>,

    pub logs_view_data: LogsViewData,
    pub changes_view_data: ChangesViewData,

    pub create_data: ContainerCreateData,
    pub rename_window: RenameWindow,
    pub container_view: ContainerView,
    pub central_view: CentralView,
}

impl ContainersTab {
    pub fn clear(&mut self) {
        self.containers.clear();
        self.clear_container();
    }

    pub fn clear_container(&mut self) {
        self.current_container = None;
        self.current_stats = None;
        self.logs_view_data.current_logs = None;
        self.logs_view_data.page = 0;
    }
}

impl App {
    pub fn link_container(&self, ui: &mut egui::Ui, id: ContainerIdRef, name: Option<&str>) {
        if ui
            .add(
                egui::Label::new(
                    RichText::new(name.map(|n| n.trim_start_matches('/')).unwrap_or(id)).strong(),
                )
                .sense(egui::Sense {
                    click: true,
                    focusable: true,
                    drag: false,
                }),
            )
            .on_hover_text("click to follow")
            .clicked()
        {
            let _ = self.send_event(EventRequest::Container(ContainerEvent::TraceStart {
                id: id.to_string(),
            }));
            let _ = self.send_event(EventRequest::NotifyGui(GuiEvent::SetTab(
                crate::app::Tab::Containers,
            )));
        }
    }

    pub fn containers_side(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            self.containers_menu(ui);
            self.containers_scroll(ui);
        });
    }

    pub fn containers_view(&mut self, ui: &mut egui::Ui) {
        match self.containers.central_view {
            CentralView::None => {}
            CentralView::Container => self.container_details(ui),
            CentralView::Create => self.container_create(ui),
        }
        self.display_rename_window(ui);
    }

    fn containers_menu(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("containers_tab_menu").show(ui, |ui| {
            ui.selectable_value(
                &mut self.containers.central_view,
                CentralView::None,
                "main view",
            );
            ui.selectable_value(
                &mut self.containers.central_view,
                CentralView::Create,
                "create",
            );
        });
        egui::Grid::new("containers_button_menu").show(ui, |ui| {
            if ui.button("prune").clicked() {
                self.popups.push_back(
                    ui::ActionPopup::builder(EventRequest::Container(ContainerEvent::Prune))
                        .title("Delete stopped containers")
                        .text("Are you sure you want to delete all stopped containers?")
                        .build(),
                );
            }
        });
    }

    fn containers_scroll(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .spacing((0., 0.))
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut error = None;
                    let mut popup = None;
                    let mut central_view = self.containers.central_view;
                    for container in &self.containers.containers {
                        let color = color_for_state(&container.state);
                        let dot = state_icon(color);
                        let frame_color = ui.visuals().widgets.open.bg_fill;
                        let selected = self
                            .containers
                            .current_container
                            .as_ref()
                            .map(|c| c.id == container.id && central_view == CentralView::Container)
                            .unwrap_or_default();

                        let frame = if selected {
                            egui::Frame::none()
                                .fill(frame_color)
                                .inner_margin(Margin::symmetric(0., 0.))
                        } else {
                            egui::Frame::none().inner_margin(Margin::symmetric(0., 0.))
                        };
                        frame.show(ui, |ui| {
                            egui::Grid::new(&container.id)
                                .spacing((0., 5.))
                                .show(ui, |ui| {
                                    ui.add(ui::line_with_size(frame, (self.side_panel_size(), 1.)));
                                    ui.end_row();
                                    egui::Grid::new(&container.id[0..8])
                                        .spacing((2.5, 5.))
                                        .max_col_width(self.side_panel_size())
                                        .show(ui, |ui| {
                                            ui.add_space(5.);
                                            ui.scope(|ui| {
                                                ui.add(dot);
                                                if let Some(name) = container.names.first() {
                                                    ui.add(
                                                        Label::new(
                                                            RichText::new(
                                                                name.trim_start_matches('/'),
                                                            )
                                                            .strong()
                                                            .heading(),
                                                        )
                                                        .wrap(true),
                                                    );
                                                } else {
                                                    ui.add(
                                                        Label::new(
                                                            RichText::new(&container.id[..12])
                                                                .strong()
                                                                .heading(),
                                                        )
                                                        .wrap(true),
                                                    );
                                                }
                                            });
                                            ui.end_row();
                                            ui.add_space(5.);
                                            self.link_image(ui, &container.image, None);
                                            ui.end_row();

                                            ui.add_space(5.);
                                            ui.add(
                                                Label::new(
                                                    RichText::new(&container.status)
                                                        .italics()
                                                        .strong(),
                                                )
                                                .wrap(true),
                                            );
                                            ui.end_row();

                                            ui.add_space(5.);
                                            ui.scope(|ui| {
                                                if ui
                                                    .button(icon::INFO)
                                                    .on_hover_text("Inspect this container")
                                                    .clicked()
                                                {
                                                    central_view = CentralView::Container;
                                                    if let Err(e) =
                                                        self.send_event(EventRequest::Container(
                                                            ContainerEvent::TraceStart {
                                                                id: container.id.clone(),
                                                            },
                                                        ))
                                                    {
                                                        error = Some(Box::new(e));
                                                    };
                                                }
                                                if ui
                                                    .button(icon::DELETE)
                                                    .on_hover_text("Delete this container")
                                                    .clicked()
                                                {
                                                    popup = Some(
                                                        ui::ActionPopup::builder(
                                                            EventRequest::Container(
                                                                ContainerEvent::Delete {
                                                                    id: container.id.clone(),
                                                                },
                                                            ),
                                                        )
                                                        .title("Delete container")
                                                        .text(format!(
                                                    "are you sure you want to delete container {}?",
                                                    &container.id
                                                ))
                                                        .build(),
                                                    );
                                                }
                                                match container.state {
                                                    ContainerStatus::Running => {
                                                        btn!(stop => self, ui, container, error);
                                                        btn!(pause => self, ui, container, error);
                                                        btn!(restart => self, ui, container, error);
                                                    }
                                                    ContainerStatus::Paused => {
                                                        btn!(stop => self, ui, container, error);
                                                        btn!(unpause => self, ui, container, error);
                                                        btn!(restart => self, ui, container, error);
                                                    }
                                                    _ => {
                                                        btn!(start => self, ui, container, error);
                                                    }
                                                }
                                            });
                                            ui.end_row();
                                        });
                                    ui.end_row();
                                    ui.scope(|_| {});
                                    ui.end_row();
                                });
                        });
                        ui.end_row();
                    }
                    if let Some(error) = error {
                        self.add_error(error);
                    }
                    if let Some(popup) = popup {
                        self.popups.push_back(popup);
                    }
                    self.containers.central_view = central_view;
                });
        });
    }

    fn container_create(&mut self, ui: &mut egui::Ui) {
        Grid::new("container_create").show(ui, |ui| {
            ui.scope(|_| {});
            ui.allocate_space((self.side_panel_size(), 0.).into());
            ui.end_row();
            key!(ui, "Image:");
            ui.text_edit_singleline(&mut self.containers.create_data.image);
            ui.end_row();
            key!(ui, "Command:");
            ui.text_edit_singleline(&mut self.containers.create_data.command);
            ui.end_row();
            key!(ui, "Name:");
            ui.text_edit_singleline(&mut self.containers.create_data.name);
            ui.end_row();
            key!(ui, "Working directory:");
            ui.text_edit_singleline(&mut self.containers.create_data.working_dir);
            ui.end_row();
            key!(ui, "User:");
            ui.text_edit_singleline(&mut self.containers.create_data.user);
            ui.end_row();
            key!(ui, "User namespace mode:");
            ui.text_edit_singleline(&mut self.containers.create_data.userns_mode);
            ui.end_row();
            key!(ui, "Network mode:");
            ui.text_edit_singleline(&mut self.containers.create_data.network_mode);
            ui.end_row();
            key!(ui, "Logging driver:");
            ui.text_edit_singleline(&mut self.containers.create_data.log_driver);
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.tty, "TTY");
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.stdin, "Standard input");
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.stdout, "Standard output");
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.stderr, "Standard error");
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.privileged, "Privileged");
            ui.end_row();
            ui.checkbox(&mut self.containers.create_data.autoremove, "Auto remove");
            ui.end_row();

            ui.end_row();

            ui.add( ui::EditableList::builder_key_val(&mut self.containers.create_data.env)
                .heading("Environment:")
                .add_hover_text(
                "Add a key value pair"
                )
                .key_heading("Key:")
                .val_heading("Val:")
                .build()
            );
            ui.end_row();

            ui.add(ui::EditableList::builder_key(&mut self.containers.create_data.sec_ops)
                .heading("Security options:")
                .add_hover_text(
                "Add a security option"
                ).build()
            );
            ui.end_row();

            ui.add(ui::EditableList::builder_key(&mut self.containers.create_data.capabilities)
                .heading("Capabilities:")
                .add_hover_text(
                "Add a capability"
                ).build()
            );
            ui.end_row();

            ui.add(ui::EditableList::builder_key(&mut self.containers.create_data.volumes)
                .heading("Volume mounts:")
                .add_hover_text(
                "Add a volume mount from host in the form of `/some/host/path:/some/container/path`)"
                ).build()
            );
            ui.end_row();

            ui.add(ui::EditableList::builder_key(&mut self.containers.create_data.links)
                .heading("Links:")
                .add_hover_text(
                "Add a link"
                ).build()
            );
            ui.end_row();

            key!(ui, "CPUs:");
            ui.add(egui::DragValue::new(&mut self.containers.create_data.cpus).speed(1.));
            ui.end_row();
            key!(ui, "CPU shares:");
            ui.add(egui::DragValue::new(&mut self.containers.create_data.cpu_shares).speed(1.));
            ui.end_row();

            if self.containers.create_data.cpus < 0. {
                self.containers.create_data.cpus = 0.;
            }
            key!(ui, "Memory:");
            ui.text_edit_singleline(&mut self.containers.create_data.memory).on_hover_text("examples between ``: `1G`, `100m`, `1000000`");
            ui.end_row();
            key!(ui, "Memory swap:");
            ui.text_edit_singleline(&mut self.containers.create_data.memory_swap);
            ui.end_row();

            ui.scope(|ui| {
                if ui.button("create").clicked() {
                    if self.containers.create_data.image.is_empty() {
                        self.add_error("Image name is required to create a container");
                    } else {
                            match self.containers.create_data.as_opts() {
                                Ok(opts) => {

                        self.send_event_notify(EventRequest::Container(ContainerEvent::Create(
                                    opts
                        )));
                                }
                                Err(e) => self.add_error(e)
                            }
                    }
                }
                ui.add_space(5.);
                if ui.button("reset").clicked() {
                    self.containers.create_data.reset();
                }
            });
        });
    }

    fn container_details(&mut self, ui: &mut egui::Ui) {
        let mut error = None;
        let mut rename_id = None;
        if let Some(container) = &self.containers.current_container {
            let color = if is_running(container) {
                egui::Color32::GREEN
            } else if is_paused(container) {
                egui::Color32::YELLOW
            } else {
                egui::Color32::RED
            };
            ui.horizontal(|ui| {
                ui.add(state_icon(color));
                ui.add(
                    Label::new(
                        RichText::new(container.name.trim_start_matches('/'))
                            .heading()
                            .strong(),
                    )
                    .wrap(true),
                );
                self.container_buttons(ui, container, &mut error);
                if ui.button("rename").clicked() {
                    rename_id = Some(container.id.clone());
                }
            });
            ui.add_space(10.);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.containers.container_view,
                    ContainerView::Details,
                    "details",
                );
                ui.selectable_value(
                    &mut self.containers.container_view,
                    ContainerView::Logs,
                    "logs",
                );
                if self.containers.changes_view_data.current_changes.is_some() {
                    ui.selectable_value(
                        &mut self.containers.container_view,
                        ContainerView::Changes,
                        "changes",
                    );
                }
                if is_running(container) {
                    ui.selectable_value(
                        &mut self.containers.container_view,
                        ContainerView::Processes,
                        "processes",
                    );
                    ui.selectable_value(
                        &mut self.containers.container_view,
                        ContainerView::Attach,
                        "attach",
                    );
                }
            });
            ui.add_space(15.);
            match self.containers.container_view {
                ContainerView::Details => {
                    self.container_info(ui, container);
                    self.container_stats(ui);
                }
                ContainerView::Logs => {
                    self.container_logs(ui);
                }
                ContainerView::Processes => self.container_processes(ui),
                ContainerView::Changes => self.container_changes(ui),
                ContainerView::Attach => {}
            }
        }
        if let Some(id) = rename_id {
            self.containers.rename_window.toggle();
            self.containers.rename_window.id = id;
        }
        if let Some(error) = error {
            self.add_error(error);
        }
    }

    fn container_buttons(
        &self,
        ui: &mut egui::Ui,
        container: &ContainerDetails,
        error: &mut Option<Box<anyhow::Error>>,
    ) {
        let mut _error = None;
        if is_running(container) {
            ui.horizontal(|ui| {
                btn!(stop => self, ui, container, _error);
                btn!(pause => self, ui, container, _error);
                btn!(restart => self, ui, container, _error);
            });
        } else if is_paused(container) {
            ui.horizontal(|ui| {
                btn!(stop => self, ui, container, _error);
                btn!(unpause => self, ui, container, _error);
                btn!(restart => self, ui, container, _error);
            });
        } else {
            ui.horizontal(|ui| {
                btn!(start => self, ui, container, _error);
            });
        }
        std::mem::swap(error, &mut _error);
    }

    fn container_info(&self, ui: &mut egui::Ui, container: &ContainerDetails) {
        Grid::new("container_info").show(ui, |ui| {
            key_val!(ui, "ID:", &container.id);

            key!(ui, "Image:");
            self.link_image(ui, &container.image, None);
            ui.end_row();

            key_val!(ui, "Created:", crate::format_date(&container.created));
            key_val!(ui, "State:", container.state.status.as_ref());

            if let Some(config) = &container.config {
                key_val!(
                    ui,
                    "Command:",
                    &config.cmd.as_deref().unwrap_or(&[]).join(" ")
                );

                if let Some(entrypoint) = config.entrypoint.as_ref() {
                    key_val!(ui, "Entrypoint:", &entrypoint.join(" "));
                }

                key!(ui, "Labels:");
                ui.end_row();
                if let Some(labels) = config.labels.as_ref() {
                    if !labels.is_empty() {
                        ui.label("          ");
                        Grid::new("labels_grid").show(ui, |ui| {
                            let mut labels = labels.iter().collect::<Vec<_>>();
                            labels.sort();
                            for (k, v) in labels {
                                val!(ui, &k);
                                val!(ui, &v);
                                ui.end_row();
                            }
                        });
                        ui.end_row();
                    }
                }

                key_val!(ui, "Hostname:", &config.hostname);

                if !config.domainname.is_empty() {
                    key_val!(ui, "Domainname:", &config.domainname);
                }

                key_val!(ui, "User:", &config.user);
                key_val!(ui, "Working dir:", &config.working_dir);

                if let Some(shell) = config.shell.as_ref() {
                    key_val!(ui, "Shell:", &shell.join(" "));
                }

                key!(ui, "Env: ");
                ui.end_row();
                if !config.env.is_empty() {
                    for var in &config.env {
                        ui.scope(|_| {});
                        val!(ui, &var);
                        ui.end_row();
                    }
                }
            }

            if let Some(size_rw) = &container.size_rw {
                key_val!(ui, "Size RW:", &format!("{}", size_rw));
            }

            if let Some(size_root_fs) = &container.size_root_fs {
                key_val!(ui, "Size rootfs:", &format!("{}", size_root_fs));
            }

            key_val!(ui, "Restart count:", container.restart_count);

            key_val!(ui, "Driver:", &container.driver);
            key_val!(ui, "Platform:", &container.platform);

            key!(ui, "Networks:");
            if !container.network_settings.networks.is_empty() {
                egui::CollapsingHeader::new("").show(ui, |ui| {
                    egui::Grid::new("networks_grid").show(ui, |ui| {
                        for (name, entry) in &container.network_settings.networks {
                            key!(ui, name.as_str());
                            ui.end_row();
                            ui.scope(|_| {});
                            egui::Grid::new(&name).show(ui, |ui| {
                                key_val!(ui, "MAC address:", &entry.mac_address);
                                key_val!(
                                    ui,
                                    "IPv4:",
                                    format!("{}/{}", entry.ip_address, entry.ip_prefix_len)
                                );
                                key_val!(ui, "Gateway:", &entry.gateway);
                                key_val!(
                                    ui,
                                    "IPv6:",
                                    format!(
                                        "{}/{}",
                                        entry.global_ipv6_address, entry.global_ipv6_prefix_len
                                    )
                                );
                                key_val!(ui, "IPv6 gateway:", &entry.ipv6_gateway);
                                key_val!(ui, "Network ID:", &entry.network_id);
                                key_val!(ui, "Endpoint ID:", &entry.endpoint_id);
                            });
                            ui.end_row();
                            ui.scope(|_| {});
                            ui.separator();
                            ui.end_row();
                        }
                    });
                });
                ui.end_row();
            }
        });
    }

    fn container_stats(&self, ui: &mut egui::Ui) {
        if let Some(stats) = &self.containers.current_stats {
            egui::CollapsingHeader::new("Stats")
                .default_open(false)
                .show(ui, |ui| {
                    let cpu_data =
                        plot::Values::from_values_iter(stats.0.iter().map(|(time, stat)| {
                            plot::Value::new(time.as_secs_f64(), stat.cpu_usage)
                        }));

                    let mem_data =
                        plot::Values::from_values_iter(stats.0.iter().map(|(time, stat)| {
                            plot::Value::new(time.as_secs_f64(), stat.mem_percent)
                        }));

                    Grid::new("stats_grid").show(ui, |ui| {
                        if let Some(last) = stats.0.last() {
                            key_val!(ui, "CPU usage:", format!("{:0.2}%", last.1.cpu_usage));

                            key_val!(
                                ui,
                                "Memory usage:",
                                format!(
                                    "{} / {}  {:0.2}%",
                                    crate::conv_fb(last.1.mem_usage),
                                    crate::conv_fb(last.1.mem_limit),
                                    last.1.mem_percent
                                )
                            );

                            if let Some(net_stat) = &last.1.net_stat {
                                let (rx, tx) =
                                    net_stat.iter().fold((0, 0), |mut acc, (_, stats)| {
                                        acc.0 += stats.rx_bytes;
                                        acc.1 += stats.tx_bytes;
                                        acc
                                    });
                                key_val!(
                                    ui,
                                    "Network I/O:",
                                    format!("{} / {}", crate::conv_b(rx), crate::conv_b(tx))
                                );
                            }

                            if let Some(blkio_stat) = &last.1.blkio_stat {
                                let (rx, tx) = blkio_stat
                                    .io_service_bytes_recursive
                                    .as_ref()
                                    .map(|stats| {
                                        stats.iter().fold((0, 0), |mut acc, stat| {
                                            match stat.op.chars().next() {
                                                Some('r' | 'R') => acc.0 += stat.value,
                                                Some('w' | 'W') => acc.1 += stat.value,
                                                _ => {}
                                            }
                                            acc
                                        })
                                    })
                                    .unwrap_or((0, 0));
                                key_val!(
                                    ui,
                                    "Disk I/O:",
                                    format!("{} / {}", crate::conv_b(rx), crate::conv_b(tx))
                                );
                            }

                            if let Some(pids_stat) = &last.1.pids_stat {
                                key_val!(
                                    ui,
                                    "Processes:",
                                    format!(
                                        "{} / {}",
                                        pids_stat.current.unwrap_or_default(),
                                        pids_stat.limit.unwrap_or_default()
                                    )
                                );
                            }

                            if let Some(mem_stat) = &last.1.mem_stat {
                                ui.add(Label::new(RichText::new("Memory stats:").strong()));
                                egui::CollapsingHeader::new("")
                                    .id_source("mem_stats")
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        Grid::new("mem_stats_grid").show(ui, |ui| {
                                            macro_rules! mem_key_val {
                                                ($k:literal, $v:expr) => {
                                                    if let Some(v) = $v {
                                                        key_val!(ui, $k, v.to_string());
                                                    }
                                                };
                                            }
                                            mem_key_val!("Cache:", mem_stat.cache);
                                            mem_key_val!("Total cache:", mem_stat.total_cache);
                                            mem_key_val!("Active files:", mem_stat.active_file);
                                            mem_key_val!(
                                                "Total active files:",
                                                mem_stat.total_active_file
                                            );
                                            mem_key_val!("Inactive files:", mem_stat.inactive_file);
                                            mem_key_val!(
                                                "Total inactive files:",
                                                mem_stat.total_inactive_file
                                            );
                                            mem_key_val!("Mapped files:", mem_stat.mapped_file);
                                            mem_key_val!(
                                                "Total mapped files:",
                                                mem_stat.total_mapped_file
                                            );
                                            mem_key_val!("Page out:", mem_stat.pgpgout);
                                            mem_key_val!("Total page out:", mem_stat.total_pgpgout);
                                            mem_key_val!("Page in:", mem_stat.pgpgin);
                                            mem_key_val!("Total page in:", mem_stat.total_pgpgin);
                                            mem_key_val!("Page faults:", mem_stat.pgfault);
                                            mem_key_val!(
                                                "Total page faults:",
                                                mem_stat.total_pgfault
                                            );
                                            mem_key_val!("Page major faults:", mem_stat.pgmajfault);
                                            mem_key_val!(
                                                "Total page major faults:",
                                                mem_stat.total_pgmajfault
                                            );
                                            mem_key_val!("Active anonymous:", mem_stat.active_anon);
                                            mem_key_val!(
                                                "Total active anonymous:",
                                                mem_stat.total_active_anon
                                            );
                                            mem_key_val!(
                                                "Inactive anonymous:",
                                                mem_stat.inactive_anon
                                            );
                                            mem_key_val!(
                                                "Total inactive anonymous:",
                                                mem_stat.total_active_anon
                                            );
                                            mem_key_val!("RSS:", mem_stat.rss);
                                            mem_key_val!("Total RSS:", mem_stat.total_rss);
                                            mem_key_val!("RSS huge:", mem_stat.rss_huge);
                                            mem_key_val!(
                                                "Total RSS huge:",
                                                mem_stat.total_rss_huge
                                            );
                                            mem_key_val!("Unevictable:", mem_stat.unevictable);
                                            mem_key_val!(
                                                "Total unevictable:",
                                                mem_stat.total_unevictable
                                            );
                                            mem_key_val!("Writeback:", mem_stat.writeback);
                                            mem_key_val!(
                                                "Total writeback:",
                                                mem_stat.total_writeback
                                            );
                                            mem_key_val!(
                                                "Hierarchical memory limit:",
                                                mem_stat.hierarchical_memory_limit
                                            );
                                            mem_key_val!(
                                                "Hierarchical memsw limit:",
                                                mem_stat.hierarchical_memsw_limit
                                            );
                                        });
                                    });
                            }
                            ui.end_row();

                            if let Some(net_stat) = &last.1.net_stat {
                                ui.add(Label::new(RichText::new("Network stats:").strong()));
                                egui::CollapsingHeader::new("")
                                    .id_source("net_stats")
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for (network, stats) in net_stat {
                                            egui::CollapsingHeader::new(network)
                                                .default_open(false)
                                                .show(ui, |ui| {
                                                    Grid::new(&network).show(ui, |ui| {
                                                        key_val!(ui, "rx_bytes", stats.rx_bytes);
                                                        key_val!(ui, "tx_bytes", stats.tx_bytes);
                                                        key_val!(
                                                            ui,
                                                            "rx_packets",
                                                            stats.rx_packets
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "tx_packets",
                                                            stats.tx_packets
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "rx_dropped",
                                                            stats.rx_dropped
                                                        );
                                                        key_val!(
                                                            ui,
                                                            "tx_dropped",
                                                            stats.tx_dropped
                                                        );
                                                        key_val!(ui, "rx_errors", stats.rx_errors);
                                                        key_val!(ui, "tx_errors", stats.tx_errors);
                                                    });
                                                });
                                        }
                                    });
                            }
                        }
                    });
                    let color = if ui.visuals().dark_mode {
                        *color::D_BG_000
                    } else {
                        *color::L_BG_4
                    };
                    Frame::none().fill(color).show(ui, |ui| {
                        Plot::new("CPU usage")
                            .data_aspect(1.5)
                            .show_background(false)
                            .height(self.graph_height())
                            .include_x(0.)
                            .include_y(0.)
                            .legend(egui::widgets::plot::Legend {
                                position: egui::widgets::plot::Corner::RightTop,
                                ..Default::default()
                            })
                            .show(ui, |ui| {
                                ui.line(
                                    Line::new(cpu_data)
                                        .name("CPU usage %")
                                        .color(egui::Color32::YELLOW),
                                )
                            });
                    });
                    Frame::none().fill(color).show(ui, |ui| {
                        Plot::new("Memory usage")
                            .data_aspect(1.5)
                            .show_background(false)
                            .height(self.graph_height())
                            .include_x(0.)
                            .include_y(0.)
                            .legend(egui::widgets::plot::Legend {
                                position: egui::widgets::plot::Corner::RightTop,
                                ..Default::default()
                            })
                            .show(ui, |ui| {
                                ui.line(
                                    Line::new(mem_data)
                                        .name("Memory usage %")
                                        .color(egui::Color32::BLUE),
                                )
                            });
                    });
                });
        }
    }

    fn container_logs(&mut self, ui: &mut egui::Ui) {
        if let Some(logs) = &self.containers.logs_view_data.current_logs {
            egui::CollapsingHeader::new("Logs")
                .default_open(false)
                .show(ui, |ui| {
                    let color = if ui.visuals().dark_mode {
                        *color::D_BG_000
                    } else {
                        *color::L_BG_4
                    };

                    let rope = ropey::Rope::from(logs.as_str());

                    let len_lines = rope.len_lines();
                    let max_page = len_lines / PAGE_SIZE;
                    let cur_line = self.containers.logs_view_data.page * PAGE_SIZE;

                    let mut slice = if self.containers.logs_view_data.follow {
                        self.containers.logs_view_data.page = max_page;
                        &logs[rope.line_to_byte(len_lines.saturating_sub(PAGE_SIZE))..]
                    } else if cur_line + PAGE_SIZE > len_lines {
                        let start_idx = rope.line_to_byte(
                            cur_line.saturating_sub(cur_line + PAGE_SIZE - len_lines),
                        );
                        &logs[start_idx..]
                    } else {
                        &logs[rope.line_to_byte(cur_line)..rope.line_to_byte(cur_line + PAGE_SIZE)]
                    };

                    let mut page = self.containers.logs_view_data.page as f32;
                    ui.horizontal(|ui| {
                        if ui
                            .button(icon::ARROW_LEFT)
                            .on_hover_text("Decrease the page")
                            .clicked()
                            && self.containers.logs_view_data.page > 0
                        {
                            self.containers.logs_view_data.page -= 1;
                        }
                        ui.add(
                            egui::DragValue::new(&mut page)
                                .clamp_range(0..=max_page)
                                .fixed_decimals(0)
                                .speed(1.),
                        );
                        if ui
                            .button(icon::ARROW_RIGHT)
                            .on_hover_text("Increase the page")
                            .clicked()
                            && self.containers.logs_view_data.page < max_page
                        {
                            self.containers.logs_view_data.page += 1;
                        }
                        ui.checkbox(&mut self.containers.logs_view_data.follow, "Follow logs");
                    });
                    self.containers.logs_view_data.page = page as usize;

                    Frame::none().fill(color).show(ui, |ui| {
                        ui.allocate_space((ui.available_rect_before_wrap().width(), 0.).into());
                        ui.text_edit_multiline(&mut slice);
                    });
                });
        }
    }

    fn display_rename_window(&mut self, ui: &mut egui::Ui) {
        if self.containers.rename_window.show {
            egui::Window::new("Rename a container").show(ui.ctx(), |ui| {
                ui.text_edit_singleline(&mut self.containers.rename_window.new_name);

                Grid::new("rename_window_buttons").show(ui, |ui| {
                    if ui.button("OK").clicked() {
                        let name = self.containers.rename_window.new_name.clone();
                        if name.is_empty() {
                            self.add_error("Name of the container can't be empty");
                        } else {
                            self.send_event_notify(EventRequest::Container(
                                ContainerEvent::Rename {
                                    id: self.containers.rename_window.id.clone(),
                                    name,
                                },
                            ));
                            self.containers.rename_window.toggle();
                        }
                    }

                    if ui.button("close").clicked() {
                        self.containers.rename_window.toggle();
                    }
                    ui.end_row();
                });
            });
        }
    }

    fn container_processes(&mut self, ui: &mut egui::Ui) {
        if let Some(top) = &self.containers.current_top {
            Grid::new("container_processes").show(ui, |ui| {
                for title in &top.titles {
                    key!(ui, title);
                }
                ui.end_row();

                for process in &top.processes {
                    for field in process {
                        val!(ui, field);
                    }
                    ui.end_row();
                }
            });
        }
    }

    fn container_changes(&mut self, ui: &mut egui::Ui) {
        if let Some(changes) = &self.containers.changes_view_data.current_changes {
            ui.allocate_space((f32::INFINITY, 0.).into());
            let max_page = changes.len() / PAGE_SIZE;
            ui.horizontal(|ui| {
                if ui
                    .button(icon::ARROW_LEFT)
                    .on_hover_text("Decrease the page")
                    .clicked()
                    && self.containers.changes_view_data.page > 0
                {
                    self.containers.changes_view_data.page -= 1;
                }
                ui.add(
                    egui::DragValue::new(&mut self.containers.changes_view_data.page)
                        .clamp_range(0..=max_page)
                        .fixed_decimals(0)
                        .speed(1.),
                );
                if ui
                    .button(icon::ARROW_RIGHT)
                    .on_hover_text("Increase the page")
                    .clicked()
                    && self.containers.changes_view_data.page < max_page
                {
                    self.containers.changes_view_data.page += 1;
                }
            });
            Grid::new("container_changes").show(ui, |ui| {
                for change in changes
                    .iter()
                    .skip(self.containers.changes_view_data.page * PAGE_SIZE)
                    .take(PAGE_SIZE)
                {
                    let label = match change.kind {
                        ChangeKind::Modified => {
                            Label::new(RichText::new("M").color(egui::Color32::YELLOW).strong())
                        }
                        ChangeKind::Added => {
                            Label::new(RichText::new("A").color(egui::Color32::GREEN).strong())
                        }
                        ChangeKind::Deleted => {
                            Label::new(RichText::new("D").color(egui::Color32::RED).strong())
                        }
                    };
                    ui.scope(|ui| {
                        ui.add(label);
                        ui.add_space(5.);
                        ui.label(&change.path);
                    });
                    ui.end_row();
                }
            });
        }
    }
}
