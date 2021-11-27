use crate::app::{key, key_val, line, val, App, DELETE_ICON, INFO_ICON, SAVE_ICON, SCROLL_ICON};
use crate::event::EventRequest;
use docker_api::api::{ImageBuildChunk, RegistryAuth};

use anyhow::Error;
use egui::{Grid, Label, TextEdit};

fn trim_id(id: &str) -> &str {
    &id.trim_start_matches("sha256:")[..12]
}

fn name(id: &str, tags: Option<&Vec<String>>) -> String {
    let id = trim_id(id);
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

#[derive(Default)]
pub struct PullView {
    pub image: String,
    pub user: String,
    pub password: String,
    pub in_progress: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImagesView {
    Image,
    Pull,
    None,
}

impl App {
    pub fn image_view(&mut self, ui: &mut egui::Ui) {
        match self.current_image_view {
            ImagesView::Image => self.image_details(ui),
            ImagesView::Pull => self.image_pull(ui),
            ImagesView::None => {}
        }
    }

    pub fn image_side(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            self.image_menu(ui);
            self.image_scroll(ui);
        });
    }

    fn image_menu(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("image_menu").show(ui, |ui| {
            ui.selectable_value(&mut self.current_image_view, ImagesView::None, "main view");
            ui.selectable_value(&mut self.current_image_view, ImagesView::Pull, "pull");
        });
    }

    fn image_scroll(&mut self, ui: &mut egui::Ui) {
        let mut view = self.current_image_view;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .spacing((0., 0.))
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut errors = vec![];
                    let color = ui.visuals().widgets.active.bg_fill;
                    for image in &self.images {
                        let frame = if self
                            .current_image
                            .as_ref()
                            .map(|i| {
                                i.details.id == image.id
                                    && self.current_image_view == ImagesView::Image
                            })
                            .unwrap_or_default()
                        {
                            egui::Frame::none().fill(color).margin((0., 0.))
                        } else {
                            egui::Frame::none().margin((0., 0.))
                        };

                        frame.show(ui, |ui| {
                            egui::Grid::new(&image.id)
                                .spacing((0., 0.))
                                .max_col_width(self.side_panel_size())
                                .show(ui, |ui| {
                                    let image_name = name(&image.id, image.repo_tags.as_ref());
                                    ui.scope(|ui| {
                                        ui.heading(SCROLL_ICON);
                                        ui.add(Label::new(&image_name).strong().wrap(true));
                                    });
                                    ui.end_row();

                                    ui.add(Label::new(&image.created.to_rfc2822()).italics());
                                    ui.end_row();

                                    ui.add(Label::new(crate::conv_b(image.virtual_size)).italics());
                                    ui.end_row();

                                    ui.scope(|ui| {
                                        if ui
                                            .button(INFO_ICON)
                                            .on_hover_text(
                                                "display detailed information about the image",
                                            )
                                            .clicked()
                                        {
                                            if let Err(e) =
                                                self.send_event(EventRequest::InspectImage {
                                                    id: image.id.clone(),
                                                })
                                            {
                                                errors.push(e);
                                            };
                                            view = ImagesView::Image;
                                        }
                                        if ui
                                            .button(DELETE_ICON)
                                            .on_hover_text("delete the image")
                                            .clicked()
                                        {
                                            if let Err(e) =
                                                self.send_event(EventRequest::DeleteImage {
                                                    id: image.id.clone(),
                                                })
                                            {
                                                errors.push(e);
                                            };
                                        }
                                        if ui
                                            .button(SAVE_ICON)
                                            .on_hover_text(
                                                "save the image to filesystem as tar archive",
                                            )
                                            .clicked()
                                        {
                                            let tar_name = format!("image_{}", trim_id(&image.id));
                                            log::warn!("{}", tar_name);
                                            match native_dialog::FileDialog::new()
                                                .add_filter("tar archive", &["tar"])
                                                .set_filename(&tar_name[..])
                                                .show_save_single_file()
                                            {
                                                Ok(Some(output_path)) => {
                                                    if let Err(e) =
                                                        self.send_event(EventRequest::SaveImage {
                                                            id: image.id.clone(),
                                                            output_path,
                                                        })
                                                    {
                                                        errors.push(e);
                                                    };
                                                }
                                                Ok(None) => {}
                                                Err(e) => errors.push(Error::msg(format!(
                                                    "failed to spawn a file dialog - {}",
                                                    e,
                                                ))),
                                            }
                                        }
                                    });
                                });
                            ui.allocate_space((ui.available_width(), 0.).into());
                        });
                        ui.end_row();
                        line(ui, frame);
                        ui.end_row();
                    }
                    errors.iter().for_each(|err| self.add_notification(err));
                });
        });
        self.current_image_view = view;
    }

    fn image_details(&self, ui: &mut egui::Ui) {
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
                //ui.allocate_space((f32::INFINITY, 0.).into());
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
                            ui.allocate_space((f32::INFINITY, 0.).into());
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

    fn image_pull(&mut self, ui: &mut egui::Ui) {
        ui.add(
            Label::new("Pull an image from a registry")
                .heading()
                .strong(),
        );
        ui.add_space(25.);

        Grid::new("image_pull_grid").show(ui, |ui| {
            ui.scope(|_| {});
            ui.allocate_space((200., 0.).into());
            ui.end_row();
            ui.add(Label::new("Image to pull:").strong());
            ui.add(TextEdit::singleline(&mut self.pull_view.image).desired_width(150.));
            ui.end_row();
            ui.add(Label::new("User:").strong());
            ui.add(TextEdit::singleline(&mut self.pull_view.user).desired_width(150.));
            ui.end_row();
            ui.add(Label::new("Password:").strong());
            ui.add(
                TextEdit::singleline(&mut self.pull_view.password)
                    .password(true)
                    .desired_width(150.),
            );
            ui.end_row();
            if ui.button("pull").clicked() {
                if self.pull_view.in_progress {
                    self.add_notification("Image pull already in progress");
                } else {
                    if self.pull_view.image.is_empty() {
                        self.add_notification("Image name can't be empty");
                    } else {
                        let auth = if !self.pull_view.user.is_empty() {
                            let mut auth = RegistryAuth::builder();
                            if !self.pull_view.password.is_empty() {
                                Some(
                                    auth.username(&self.pull_view.user)
                                        .password(&self.pull_view.password)
                                        .build(),
                                )
                            } else {
                                Some(auth.username(&self.pull_view.user).build())
                            }
                        } else {
                            None
                        };
                        self.send_event_notify(EventRequest::PullImage {
                            image: self.pull_view.image.clone(),
                            auth,
                        });
                        self.pull_view.in_progress = true;
                        self.current_pull_chunks = None;
                    }
                }
            }
        });
        let text = self
            .current_pull_chunks
            .as_ref()
            .map(|chunks| {
                chunks.iter().fold(String::new(), |mut acc, chunk| {
                    match chunk {
                        ImageBuildChunk::Update { stream } => {
                            acc.push_str("Update: ");
                            acc.push_str(&stream);
                        }
                        ImageBuildChunk::Error { error, .. } => {
                            acc.push_str("Error: ");
                            acc.push_str(&error);
                        }
                        ImageBuildChunk::Digest { aux } => {
                            acc.push_str("Digest: ");
                            acc.push_str(&aux.id);
                        }
                        ImageBuildChunk::PullStatus {
                            status,
                            id: _,
                            progress: _,
                            progress_detail,
                        } => {
                            acc.push_str("Status: ");
                            acc.push_str(&status);
                            if let Some(progress) = progress_detail {
                                if let Some(current) = progress.current {
                                    if let Some(total) = progress.total {
                                        acc.push_str(&format!(
                                            " ({} / {})",
                                            crate::conv_b(current),
                                            crate::conv_b(total)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    acc.push('\n');
                    acc
                })
            })
            .unwrap_or_default();
        ui.add(
            TextEdit::multiline(&mut text.as_str())
                .code_editor()
                .desired_width(f32::INFINITY),
        );
    }
}
