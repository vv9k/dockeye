use crate::app::{
    ui,
    ui::icon,
    ui::{key, key_val, val},
    App,
};
use crate::event::{EventRequest, NetworkEvent};
use crate::format_date;

use docker_api::api::{Ipam, NetworkCreateOpts, NetworkInfo};

use egui::{Grid, Label};

pub fn icon() -> Label {
    Label::new(icon::NETWORK).heading().strong()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CentralView {
    Network,
    Create,
    None,
}

impl Default for CentralView {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug)]
pub struct CreateViewData {
    pub name: String,
    pub driver: String,
    pub internal: bool,
    pub attachable: bool,
    pub ingress: bool,
    pub enable_ipv6: bool,
    pub opts: Vec<(String, String)>,
    pub labels: Vec<(String, String)>,
    pub ipam_driver: String,
    pub ipam_opts: Vec<(String, String)>,
    pub ipam_config: Vec<Vec<(String, String)>>,
}

impl Default for CreateViewData {
    fn default() -> Self {
        Self {
            driver: "bridge".to_string(),
            name: "".to_string(),
            internal: false,
            attachable: true,
            ingress: false,
            enable_ipv6: false,
            opts: vec![],
            labels: vec![],
            ipam_driver: "".to_string(),
            ipam_opts: vec![],
            ipam_config: vec![],
        }
    }
}

#[derive(Debug, Default)]
pub struct NetworksTab {
    pub networks: Vec<NetworkInfo>,
    pub current_network: Option<NetworkInfo>,
    pub central_view: CentralView,
    pub create_view_data: CreateViewData,
}

impl App {
    pub fn networks_view(&mut self, ui: &mut egui::Ui) {
        match self.networks.central_view {
            CentralView::Network => self.network_details(ui),
            CentralView::Create => self.network_create(ui),
            CentralView::None => {}
        }
    }

    pub fn networks_side(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            self.networks_menu(ui);
            self.networks_scroll(ui);
        });
    }

    fn networks_menu(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("networks_tab_grid").show(ui, |ui| {
            ui.selectable_value(
                &mut self.networks.central_view,
                CentralView::None,
                "main view",
            );
            ui.selectable_value(
                &mut self.networks.central_view,
                CentralView::Create,
                "create",
            );
        });
        egui::Grid::new("networks_button_grid").show(ui, |ui| {
            if ui.button("prune").clicked() {
                self.popups.push_back(ui::ActionPopup::builder(
                    EventRequest::Network(NetworkEvent::Prune)).title(
                    "Prune networks").text(
                    "Are you sure you want to prune unused networks? This will delete all networks not in use by a container.",
                ).build());
            }
        });
    }

    fn networks_scroll(&mut self, ui: &mut egui::Ui) {
        let mut view = self.networks.central_view;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .spacing((0., 0.))
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut popup = None;
                    let color = ui.visuals().widgets.open.bg_fill;
                    for network in &self.networks.networks {
                        let selected = self
                            .networks
                            .current_network
                            .as_ref()
                            .map(|i| {
                                i.id == network.id
                                    && self.networks.central_view == CentralView::Network
                            })
                            .unwrap_or_default();

                        let frame = if selected {
                            egui::Frame::none().fill(color).margin((0., 0.))
                        } else {
                            egui::Frame::none().margin((0., 0.))
                        };
                        let size = self.side_panel_size();

                        frame.show(ui, |ui| {
                            egui::Grid::new(&network.id).spacing((0., 5.)).show(ui, |ui| {
                                ui::line_with_size(ui, frame, (size, 1.));
                                ui.end_row();
                                egui::Grid::new(&network.id[0..8])
                                    .spacing((2.5, 5.))
                                    .max_col_width(size)
                                    .show(ui, |ui| {
                                        ui.add_space(5.);
                                        ui.scope(|ui| {
                                            let name = if let Some(name) = network.name.as_deref() {
                                                name
                                            } else {
                                                &network.id
                                            };
                                            ui.add(icon());
                                            ui.add(
                                                Label::new(&name)
                                                    .heading()
                                                    .strong()
                                                    .wrap(true),
                                            );
                                        });
                                        ui.end_row();

                                        ui.add_space(5.);
                                        ui.add(
                                            Label::new(format_date(&network.created))
                                                .italics()
                                                .strong()
                                                .wrap(true),
                                        );
                                        ui.end_row();

                                        ui.add_space(5.);
                                        ui.scope(|ui| {
                                            if ui
                                                .button(icon::INFO)
                                                .on_hover_text(
                                                    "display detailed information about the network",
                                                )
                                                .clicked()
                                            {
                                                self.networks.current_network = Some(network.clone());
                                                view = CentralView::Network;
                                            }
                                            if ui
                                                .button(icon::DELETE)
                                                .on_hover_text("delete the network")
                                                .clicked()
                                            {
                                                popup = Some(ui::ActionPopup::builder(
                                                    EventRequest::Network(NetworkEvent::Delete {
                                                        id: network.id.clone(),
                                                    })).title(
                                                    "Delete network").text(
                                                    format!(
                                                        "Are you sure you want to delete network {}",
                                                        &network.id
                                                    ),
                                                ).build());
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
                    if let Some(popup) = popup {
                        self.popups.push_back(popup);
                    }
                });
        });
        self.networks.central_view = view;
    }

    fn network_details(&mut self, ui: &mut egui::Ui) {
        if let Some(network) = &self.networks.current_network {
            ui.allocate_space((f32::INFINITY, 0.).into());
            let name = if let Some(name) = network.name.as_deref() {
                name
            } else {
                &network.id
            };

            ui.horizontal(|ui| {
                ui.add(icon());
                ui.add(Label::new(name).heading().wrap(true).strong());
            });
            ui.add_space(25.);

            Grid::new("network_details").show(ui, |ui| {
                if network.name.is_some() {
                    key_val!(ui, "ID:", &network.id);
                }
                key!(ui, "Labels:");
                ui.end_row();
                if !network.labels.is_empty() {
                    ui.label("          ");
                    Grid::new("labels_grid").show(ui, |ui| {
                        let mut labels = network.labels.iter().collect::<Vec<_>>();
                        labels.sort();
                        for (k, v) in labels {
                            val!(ui, &k);
                            val!(ui, &v);
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
                if let Some(scope) = &network.scope {
                    key_val!(ui, "Scope:", scope);
                }
                if let Some(driver) = &network.driver {
                    key_val!(ui, "Driver:", driver);
                }
                if let Some(enable_ipv6) = &network.enable_ipv6 {
                    key_val!(ui, "IPv6 enabled:", enable_ipv6);
                }
                if let Some(internal) = &network.internal {
                    key_val!(ui, "Internal:", internal);
                }
                if let Some(attachable) = &network.attachable {
                    key_val!(ui, "Attachable:", attachable);
                }
                if let Some(ipam) = &network.ipam {
                    key!(ui, "IPAM:");
                    egui::CollapsingHeader::new("")
                        .id_source("ipam")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("ipam_grid").show(ui, |ui| {
                                if let Some(driver) = &ipam.driver {
                                    key_val!(ui, "Driver:", driver);
                                }
                                if let Some(configs) = &ipam.config {
                                    if !configs.is_empty() {
                                        key!(ui, "Configs:");
                                        ui.end_row();
                                        ui.label("          ");
                                        Grid::new("ipam_config_grid").show(ui, |ui| {
                                            for config in configs {
                                                let mut config = config.iter().collect::<Vec<_>>();
                                                config.sort();
                                                for (k, v) in config {
                                                    val!(ui, &k);
                                                    val!(ui, &v);
                                                    ui.end_row();
                                                }
                                            }
                                        });
                                        ui.end_row();
                                    }
                                }
                                if let Some(options) = &ipam.options {
                                    if !options.is_empty() {
                                        key!(ui, "Options:");
                                        ui.end_row();
                                        ui.label("          ");
                                        Grid::new("ipam_options_grid").show(ui, |ui| {
                                            let mut options = options.iter().collect::<Vec<_>>();
                                            options.sort();
                                            for (k, v) in options {
                                                val!(ui, &k);
                                                val!(ui, &v);
                                                ui.end_row();
                                            }
                                        });
                                        ui.end_row();
                                    }
                                }
                            });
                        });
                    ui.end_row();
                }
                if let Some(options) = &network.options {
                    if !options.is_empty() {
                        key!(ui, "Options:");
                        ui.end_row();
                        ui.label("          ");
                        Grid::new("options_grid").show(ui, |ui| {
                            let mut options = options.iter().collect::<Vec<_>>();
                            options.sort();
                            for (k, v) in options {
                                val!(ui, &k);
                                val!(ui, &v);
                                ui.end_row();
                            }
                        });
                        ui.end_row();
                    }
                }
            });
        }
    }

    fn network_create(&mut self, ui: &mut egui::Ui) {
        ui.allocate_space((f32::INFINITY, 0.).into());

        ui.add(
            Label::new("Create a new network")
                .heading()
                .wrap(true)
                .strong(),
        );

        Grid::new("create_network_grid").show(ui, |ui| {
            ui.scope(|_| {});
            ui.allocate_space((self.side_panel_size(), 0.).into());
            ui.end_row();
            key!(ui, "Name:");
            ui.text_edit_singleline(&mut self.networks.create_view_data.name);
            ui.end_row();
            key!(ui, "Driver:");
            ui.text_edit_singleline(&mut self.networks.create_view_data.driver);
            ui.end_row();

            ui.add(
                ui::EditableList::builder_key_val(&mut self.networks.create_view_data.labels)
                    .heading("Labels:")
                    .build(),
            );
            ui.end_row();

            ui.add(
                ui::EditableList::builder_key_val(&mut self.networks.create_view_data.opts)
                    .heading("Options:")
                    .build(),
            );
            ui.end_row();
            ui.end_row();

            key!(ui, "IPAM Driver:");
            ui.text_edit_singleline(&mut self.networks.create_view_data.ipam_driver);
            ui.end_row();
            ui.add(
                ui::EditableList::builder_key_val(&mut self.networks.create_view_data.ipam_opts)
                    .heading("IPAM Options:")
                    .build(),
            );
            ui.end_row();
            key!(ui, "IPAM Config:");
            if ui.button(icon::ADD).clicked() {
                self.networks
                    .create_view_data
                    .ipam_config
                    .push(Default::default());
            }
            ui.end_row();
            ui.scope(|_| {});
            let mut to_delete = None;
            Grid::new("ipam_configs_grid").show(ui, |ui| {
                for (i, config) in self
                    .networks
                    .create_view_data
                    .ipam_config
                    .iter_mut()
                    .enumerate()
                {
                    let name = format!("Config {}", i);
                    ui.scope(|ui| {
                        if ui.button(icon::DELETE).clicked() {
                            to_delete = Some(i);
                        }
                        key!(ui, &name);
                    });
                    ui.end_row();
                    ui.add(
                        ui::EditableList::builder_key_val(config)
                            .heading(&name)
                            .build(),
                    );
                    ui.end_row();
                }
            });
            if let Some(idx) = to_delete {
                self.networks.create_view_data.ipam_config.remove(idx);
            }
        });

        ui.checkbox(&mut self.networks.create_view_data.internal, "Internal");
        ui.checkbox(&mut self.networks.create_view_data.attachable, "Attachable");
        ui.checkbox(&mut self.networks.create_view_data.ingress, "Ingress");
        ui.checkbox(
            &mut self.networks.create_view_data.enable_ipv6,
            "Enable IPv6",
        );

        if ui.button("create").clicked() {
            self._create_network();
        }
    }

    fn _create_network(&mut self) {
        if self.networks.create_view_data.name.is_empty() {
            self.add_error("cannot create a network without a name");
            return;
        }
        let data = &self.networks.create_view_data;
        let mut opts = NetworkCreateOpts::builder(&data.name);

        if !data.driver.is_empty() {
            opts = opts.driver(&data.driver);
        }
        if !data.labels.is_empty() {
            opts = opts.labels(data.labels.clone());
        }
        if !data.opts.is_empty() {
            opts = opts.options(data.opts.clone());
        }
        let mut ipam = Ipam {
            driver: None,
            config: None,
            options: None,
        };
        let mut set_ipam = false;
        if !data.ipam_driver.is_empty() {
            ipam.driver = Some(data.ipam_driver.clone());
            set_ipam = true;
        }
        if !data.ipam_opts.is_empty() {
            ipam.options = Some(data.ipam_opts.clone().into_iter().collect());
            set_ipam = true;
        }
        if !data.ipam_config.is_empty() {
            ipam.config = Some(
                data.ipam_config
                    .clone()
                    .into_iter()
                    .map(|d| d.into_iter().collect())
                    .collect(),
            );
            set_ipam = true;
        }
        if set_ipam {
            opts = opts.ipam(ipam);
        }

        opts = opts.attachable(data.attachable);
        opts = opts.internal(data.internal);
        opts = opts.ingress(data.ingress);
        opts = opts.enable_ipv6(data.enable_ipv6);

        self.send_event_notify(EventRequest::Network(NetworkEvent::Create(opts.build())));
    }
}
