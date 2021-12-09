use crate::app::{
    ui,
    ui::icon,
    ui::{key, key_val, val},
    App,
};
use crate::event::{EventRequest, NetworkEvent};
use crate::format_date;

use docker_api::api::NetworkInfo;

use egui::{Grid, Label};

pub fn icon() -> Label {
    Label::new(icon::NETWORK).heading().strong()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CentralView {
    Network,
    None,
}

impl Default for CentralView {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Default)]
pub struct NetworksTab {
    pub networks: Vec<NetworkInfo>,
    pub current_network: Option<NetworkInfo>,
    pub central_view: CentralView,
}

impl App {
    pub fn networks_view(&mut self, ui: &mut egui::Ui) {
        match self.networks.central_view {
            CentralView::Network => self.network_details(ui),
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
        });
        egui::Grid::new("networks_button_grid").show(ui, |ui| {
            if ui.button("prune").clicked() {
                self.popups.push_back(ui::ActionPopup::new(
                    EventRequest::Network(NetworkEvent::Prune),
                    "Prune networks",
                    "Are you sure you want to prune unused networks? This will delete all networks not in use by a container.",
                ));
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
                    //let mut errors = vec![];
                    let mut popups = vec![];
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
                                                popups.push(ui::ActionPopup::new(
                                                    EventRequest::Network(NetworkEvent::Delete {
                                                        id: network.id.clone(),
                                                    }),
                                                    "Delete network",
                                                    format!(
                                                        "Are you sure you want to delete network {}",
                                                        &network.id
                                                    ),
                                                ));
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
                    //errors.iter().for_each(|err| self.add_notification(err));
                    self.popups.extend(popups);
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
}
