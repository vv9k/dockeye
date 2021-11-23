use crate::app::{key, key_val, val, App, DELETE_ICON, INFO_ICON, SCROLL_ICON};
use crate::event::EventRequest;

use egui::{Grid, Label};

fn name(id: &str, tags: Option<&Vec<String>>) -> String {
    let id = &id.trim_start_matches("sha256:")[..12];
    if let Some(tag) = tags.and_then(|v| v.first()) {
        if tag.contains("<none>") {
            id.to_string()
        } else {
            tag.clone()
        }
    } else {
        id.to_string()
    }
}

impl App {
    pub fn image_scroll(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut errors = vec![];
                    for image in &self.images {
                        egui::Grid::new(&image.id)
                            .max_col_width(self.side_panel_size())
                            .show(ui, |ui| {
                                ui.scope(|ui| {
                                    ui.heading(SCROLL_ICON);
                                    ui.add(
                                        Label::new(name(&image.id, image.repo_tags.as_ref()))
                                            .strong()
                                            .wrap(true),
                                    );
                                });
                                ui.end_row();

                                ui.add(Label::new(&image.created.to_rfc2822()).italics());
                                ui.end_row();

                                ui.add(Label::new(crate::conv_b(image.virtual_size)).italics());
                                ui.end_row();

                                ui.scope(|ui| {
                                    if ui.button(INFO_ICON).clicked() {
                                        if let Err(e) =
                                            self.send_event(EventRequest::InspectImage {
                                                id: image.id.clone(),
                                            })
                                        {
                                            errors.push(e);
                                        };
                                    }
                                    if ui.button(DELETE_ICON).clicked() {
                                        if let Err(e) = self.send_event(EventRequest::DeleteImage {
                                            id: image.id.clone(),
                                        }) {
                                            errors.push(e);
                                        };
                                    }
                                });
                            });
                        ui.end_row();
                        ui.separator();
                        ui.end_row();
                    }
                    errors.iter().for_each(|err| self.add_notification(err));
                });
        });
    }

    pub fn image_details(&self, ui: &mut egui::Ui) {
        if let Some(image) = &self.current_image {
            let details = &image.details;

            ui.add(
                Label::new(name(&details.id, Some(details.repo_tags.as_ref())))
                    .heading()
                    .wrap(true)
                    .strong(),
            );
            ui.add_space(25.);

            Grid::new("image_details").show(ui, |ui| {
                key!(ui, "Tags:");
                ui.end_row();
                if !details.repo_tags.is_empty() {
                    ui.scope(|_| {});
                    Grid::new("tags_grid").show(ui, |ui| {
                        for tag in &details.repo_tags {
                            val!(ui, tag);
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                key_val!(ui, "Parent:", &details.parent);
                key_val!(ui, "Comment:", &details.comment);
                key_val!(ui, "Author:", &details.author);
                key_val!(ui, "Created:", &details.created.to_rfc2822());
                key_val!(ui, "Architecture:", &details.architecture);
                key_val!(
                    ui,
                    "OS:",
                    format!(
                        "{} {}",
                        details.os,
                        details.os_version.as_deref().unwrap_or_default()
                    )
                );
                key_val!(ui, "Size:", crate::conv_b(details.size as u64));
                key_val!(
                    ui,
                    "Virtual size:",
                    crate::conv_b(details.virtual_size as u64)
                );
                key_val!(ui, "Docker version:", &details.docker_version);

                key!(ui, "Digests:");
                if !details.repo_digests.is_empty() {
                    egui::CollapsingHeader::new("")
                        .id_source("digests_header")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("digests_grid").show(ui, |ui| {
                                for digest in &details.repo_digests {
                                    val!(ui, digest);
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        });
                }
                ui.end_row();

                if let Some(distribution_info) = image.distribution_info.as_ref() {
                    let descriptor = &distribution_info.descriptor;
                    key!(ui, "Descriptor:");
                    ui.end_row();
                    ui.scope(|_| {});
                    Grid::new("descriptor_grid").show(ui, |ui| {
                        key_val!(ui, "Media type:", &descriptor.media_type);
                        key_val!(ui, "Digest:", &descriptor.digest);
                        key_val!(ui, "Size:", &crate::conv_b(descriptor.size));
                        if !descriptor.urls.is_empty() {
                            key!(ui, "URLs:");
                            ui.end_row();
                            ui.scope(|_| {});
                            Grid::new("urls_grid").show(ui, |ui| {
                                for url in &descriptor.urls {
                                    val!(ui, url);
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                key!(ui, "History:");
                egui::CollapsingHeader::new("")
                    .id_source("history_header")
                    .default_open(false)
                    .show(ui, |ui| {
                        Grid::new("history_grid").show(ui, |ui| {
                            for history in &image.history {
                                key_val!(ui, "ID:", &history.id);
                                key_val!(ui, "Created:", &history.created.to_rfc2822());
                                key_val!(ui, "Size:", &crate::conv_b(history.size as u64));
                                key_val!(ui, "Comment:", &history.comment);
                                if let Some(tags) = history.tags.as_ref() {
                                    if !tags.is_empty() {
                                        key!(ui, "Tags:");
                                        ui.end_row();
                                        ui.scope(|_| {});
                                        Grid::new("history_tags_grid").show(ui, |ui| {
                                            for tag in tags {
                                                val!(ui, tag);
                                                ui.end_row();
                                            }
                                        });
                                        ui.end_row();
                                    }
                                }
                                ui.scope(|_| {});
                                ui.separator();
                                ui.end_row();
                            }
                        });
                    });
                ui.end_row();
            });
        }
    }
}
