use crate::app::{App, DELETE_ICON, INFO_ICON, SCROLL_ICON};
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
            egui::Grid::new("side_panel").show(ui, |ui| {
                let mut errors = vec![];
                for image in &self.images {
                    ui.scope(|ui| {
                        ui.label(SCROLL_ICON);
                        ui.add(Label::new(name(&image.id, image.repo_tags.as_ref())).strong());
                    });

                    ui.scope(|ui| {
                        if ui.button(INFO_ICON).clicked() {
                            if let Err(e) = self.send_event(EventRequest::InspectImage {
                                id: image.id.clone(),
                            }) {
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
                    ui.end_row();

                    ui.add(Label::new(&image.created.to_rfc2822()).italics());
                    ui.end_row();

                    ui.add(Label::new(crate::conv_b(image.virtual_size)).italics());
                    ui.end_row();
                }
                errors.iter().for_each(|err| self.add_notification(err));
            });
        });
    }

    pub fn image_details(&self, ui: &mut egui::Ui) {
        if let Some(image) = &self.current_image {
            let details = &image.details;
            ui.heading(name(&details.id, Some(&details.repo_tags)));

            Grid::new("image_details").show(ui, |ui| {
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

                key!("Tags:");
                ui.end_row();
                if !details.repo_tags.is_empty() {
                    ui.scope(|_| {});
                    Grid::new("tags_grid").show(ui, |ui| {
                        for tag in &details.repo_tags {
                            ui.add(Label::new(tag).monospace());
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                key!("Parent:");
                val!(&details.parent);
                ui.end_row();

                key!("Comment:");
                val!(&details.comment);
                ui.end_row();

                key!("Author:");
                val!(&details.author);
                ui.end_row();

                key!("Created:");
                val!(&details.created.to_rfc2822());
                ui.end_row();

                key!("Architecture:");
                val!(&details.architecture);
                ui.end_row();

                key!("OS:");
                val!(&details.os);
                ui.end_row();

                key!("OS version:");
                val!(details.os_version.as_deref().unwrap_or_default());
                ui.end_row();

                key!("Size:");
                val!(crate::conv_b(details.size as u64));
                ui.end_row();

                key!("Virtual size:");
                val!(crate::conv_b(details.virtual_size as u64));
                ui.end_row();

                key!("Docker version:");
                val!(&details.docker_version);
                ui.end_row();

                key!("Digests:");
                ui.end_row();
                if !details.repo_digests.is_empty() {
                    ui.scope(|_| {});
                    Grid::new("digests_grid").show(ui, |ui| {
                        for digest in &details.repo_digests {
                            ui.add(Label::new(digest).monospace());
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                if let Some(distribution_info) = image.distribution_info.as_ref() {
                    let descriptor = &distribution_info.descriptor;
                    key!("Descriptor:");
                    ui.end_row();
                    ui.scope(|_| {});
                    Grid::new("descriptor_grid").show(ui, |ui| {
                        ui.add(Label::new("Media type:").strong());
                        ui.add(Label::new(&descriptor.media_type).monospace());
                        ui.end_row();
                        ui.add(Label::new("Digest:").strong());
                        ui.add(Label::new(&descriptor.digest).monospace());
                        ui.end_row();
                        ui.add(Label::new("Size:").strong());
                        ui.add(Label::new(&crate::conv_b(descriptor.size)).monospace());
                        ui.end_row();
                        if !descriptor.urls.is_empty() {
                            ui.add(Label::new("URLs:").strong());
                            ui.end_row();
                            ui.scope(|_| {});
                            Grid::new("urls_grid").show(ui, |ui| {
                                for url in &descriptor.urls {
                                    ui.add(Label::new(&url).monospace());
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }

                key!("History:");
                ui.end_row();
                ui.scope(|_| {});
                Grid::new("history_grid").show(ui, |ui| {
                    for history in &image.history {
                        ui.add(Label::new("Id:").strong());
                        ui.add(Label::new(&history.id).monospace());
                        ui.end_row();
                        ui.add(Label::new("Created:").strong());
                        ui.add(Label::new(&history.created.to_rfc2822()).monospace());
                        ui.end_row();
                        ui.add(Label::new("Size:").strong());
                        ui.add(Label::new(&crate::conv_b(history.size as u64)).monospace());
                        ui.end_row();
                        ui.add(Label::new("Comment:").strong());
                        ui.add(Label::new(&history.comment).monospace());
                        ui.end_row();
                        if let Some(tags) = history.tags.as_ref() {
                            if !tags.is_empty() {
                                ui.add(Label::new("Tags:").strong());
                                ui.end_row();
                                ui.scope(|_| {});
                                Grid::new("history_tags_grid").show(ui, |ui| {
                                    for tag in tags {
                                        ui.add(Label::new(&tag).monospace());
                                        ui.end_row();
                                    }
                                });
                                ui.end_row();
                            }
                        }
                    }
                });
            });
        }
    }
}
