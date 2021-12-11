use crate::app::{
    ui,
    ui::icon,
    ui::{key, key_val, val},
    App,
};
use crate::event::{EventRequest, VolumeEvent};
use crate::format_date;

use docker_api::api::{VolumeCreateOpts, VolumeInfo, VolumesInfo};

use egui::{Grid, Label};

pub fn icon() -> Label {
    Label::new(icon::DISK).heading().strong()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CentralView {
    Volume,
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
    pub driver_opts: Vec<(String, String)>,
    pub opts: Vec<(String, String)>,
    pub labels: Vec<(String, String)>,
}

impl Default for CreateViewData {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            driver: "local".to_string(),
            driver_opts: vec![],
            opts: vec![],
            labels: vec![],
        }
    }
}

#[derive(Debug, Default)]
pub struct VolumesTab {
    pub volumes: Option<Box<VolumesInfo>>,
    pub current_volume: Option<VolumeInfo>,
    pub central_view: CentralView,
    pub create_view_data: CreateViewData,
}

impl App {
    pub fn volumes_view(&mut self, ui: &mut egui::Ui) {
        match self.volumes.central_view {
            CentralView::Volume => self.volume_details(ui),
            CentralView::Create => self.volume_create(ui),
            CentralView::None => {}
        }
    }

    pub fn volumes_side(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            self.volumes_menu(ui);
            self.volumes_scroll(ui);
        });
    }

    fn volumes_menu(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("volumes_tab_grid").show(ui, |ui| {
            ui.selectable_value(
                &mut self.volumes.central_view,
                CentralView::None,
                "main view",
            );
            ui.selectable_value(
                &mut self.volumes.central_view,
                CentralView::Create,
                "create",
            );
        });
        egui::Grid::new("volumes_button_grid").show(ui, |ui| {
            if ui.button("prune").clicked() {
                self.popups.push_back(ui::ActionPopup::new(
                    EventRequest::Volume(VolumeEvent::Prune(None)),
                    "Prune volumes",
                    "Are you sure you want to prune unused volumes? This will delete all volumes not in use by a container.",
                ));
            }
        });
    }

    fn volumes_scroll(&mut self, ui: &mut egui::Ui) {
        let mut view = self.volumes.central_view;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .spacing((0., 0.))
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut popups = vec![];
                    let color = ui.visuals().widgets.open.bg_fill;
                    if let Some(volumes) = &self.volumes.volumes {
                        for volume in &volumes.volumes {
                            let selected = self
                                .volumes
                                .current_volume
                                .as_ref()
                                .map(|i| {
                                    i.name == volume.name
                                        && self.volumes.central_view == CentralView::Volume
                                })
                                .unwrap_or_default();

                            let frame = if selected {
                                egui::Frame::none().fill(color).margin((0., 0.))
                            } else {
                                egui::Frame::none().margin((0., 0.))
                            };
                            let size = self.side_panel_size();

                            frame.show(ui, |ui| {
                                egui::Grid::new(&volume.name)
                                    .spacing((0., 5.))
                                    .show(ui, |ui| {
                                        ui::line_with_size(ui, frame, (size, 1.));
                                        ui.end_row();
                                        egui::Grid::new(&volume.mountpoint)
                                            .spacing((2.5, 5.))
                                            .max_col_width(size)
                                            .show(ui, |ui| {
                                                ui.add_space(5.);
                                                ui.scope(|ui| {
                                                    ui.add(icon());
                                                    ui.add(
                                                        Label::new(&volume.name)
                                                            .heading()
                                                            .strong()
                                                            .wrap(true),
                                                    );
                                                });
                                                ui.end_row();

                                                ui.add_space(5.);
                                                ui.add(
                                                    Label::new(format_date(&volume.created_at))
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
                                                    "display detailed information about the volume",
                                                )
                                                .clicked()
                                            {
                                                self.volumes.current_volume = Some(volume.clone());
                                                view = CentralView::Volume;
                                            }
                                                    if ui
                                                        .button(icon::DELETE)
                                                        .on_hover_text("delete the volume")
                                                        .clicked()
                                                    {
                                                        popups.push(ui::ActionPopup::new(
                                                            EventRequest::Volume(
                                                                VolumeEvent::Delete {
                                                                    id: volume.name.clone(),
                                                                },
                                                            ),
                                                            "Delete volume",
                                                            format!(
                                                        "Are you sure you want to delete volume {}",
                                                        &volume.name
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
                    }
                    self.popups.extend(popups);
                });
        });
        self.volumes.central_view = view;
    }

    fn volume_details(&mut self, ui: &mut egui::Ui) {
        if let Some(volume) = &self.volumes.current_volume {
            ui.allocate_space((f32::INFINITY, 0.).into());

            ui.horizontal(|ui| {
                ui.add(icon());
                ui.add(Label::new(&volume.name).heading().wrap(true).strong());
            });
            ui.add_space(25.);

            Grid::new("volume_details").show(ui, |ui| {
                key!(ui, "Labels:");
                ui.end_row();
                if let Some(labels) = &volume.labels {
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
                key_val!(ui, "Mountpoint:", &volume.mountpoint);
                key_val!(ui, "Scope:", &volume.scope);
                key_val!(ui, "Driver:", &volume.driver);
                if let Some(options) = &volume.options {
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

    fn volume_create(&mut self, ui: &mut egui::Ui) {
        ui.allocate_space((f32::INFINITY, 0.).into());

        ui.add(
            Label::new("Create a new volume")
                .heading()
                .wrap(true)
                .strong(),
        );

        Grid::new("create_volume_grid").show(ui, |ui| {
            ui.scope(|_| {});
            ui.allocate_space((self.side_panel_size(), 0.).into());
            ui.end_row();
            key!(ui, "Name:");
            ui.text_edit_singleline(&mut self.volumes.create_view_data.name);
            ui.end_row();
            key!(ui, "Driver:");
            ui.text_edit_singleline(&mut self.volumes.create_view_data.driver);
            ui.end_row();
            ui::keyval_grid(
                ui,
                "Driver options:",
                &mut self.volumes.create_view_data.opts,
                None,
                None::<&str>,
            );
            ui.end_row();
            ui::keyval_grid(
                ui,
                "Labels:",
                &mut self.volumes.create_view_data.labels,
                None,
                None::<&str>,
            );
            ui.end_row();
            ui::keyval_grid(
                ui,
                "Options:",
                &mut self.volumes.create_view_data.opts,
                None,
                None::<&str>,
            );
            ui.end_row();
        });

        if ui.button("create").clicked() {
            self._create_volume();
        }
    }

    fn _create_volume(&mut self) {
        let data = &self.volumes.create_view_data;
        let mut opts = VolumeCreateOpts::builder();
        if !data.name.is_empty() {
            opts = opts.name(data.name.clone());
        }
        if !data.driver.is_empty() {
            opts = opts.driver(data.driver.clone());
        }
        if !data.labels.is_empty() {
            opts = opts.labels(data.labels.clone());
        }
        if !data.name.is_empty() {
            opts = opts.driver_opts(data.driver_opts.clone());
        }
        self.send_event_notify(EventRequest::Volume(VolumeEvent::Create(opts.build())));
    }
}
