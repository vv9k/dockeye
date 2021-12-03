use crate::app::{
    ui,
    ui::icon,
    ui::{key, key_val, val},
    App,
};
use crate::event::{EventRequest, GuiEvent, ImageEvent};
use crate::ImageInspectInfo;
use docker_api::api::{ImageBuildChunk, ImageIdRef, ImageInfo, RegistryAuth, SearchResult};

use anyhow::Error;
use egui::{Grid, Label, TextEdit};

pub fn trim_id(id: &str) -> &str {
    if id.len() > 12 {
        &id.trim_start_matches("sha256:")[..12]
    } else {
        id
    }
}

pub fn icon() -> Label {
    Label::new(icon::SCROLL).heading().strong()
}

fn name(id: ImageIdRef, tags: Option<&Vec<String>>) -> String {
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

#[derive(Debug, Default)]
pub struct SearchView {
    pub image: String,
    pub images: Option<Vec<SearchResult>>,
    pub pull_in_progress: bool,
}

#[derive(Debug, Default)]
pub struct PullView {
    pub image: String,
    pub user: String,
    pub password: String,
    pub in_progress: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CentralView {
    Image,
    Pull,
    Search,
    None,
}

#[derive(Debug)]
pub struct ImagesTab {
    pub images: Vec<ImageInfo>,
    pub current_image: Option<Box<ImageInspectInfo>>,
    pub current_pull_chunks: Option<Vec<ImageBuildChunk>>,
    pub central_view: CentralView,
    pub pull_view: PullView,
    pub search_view: SearchView,
}
impl Default for ImagesTab {
    fn default() -> Self {
        Self {
            images: vec![],
            current_image: None,
            current_pull_chunks: None,
            central_view: CentralView::None,
            pull_view: PullView::default(),
            search_view: SearchView::default(),
        }
    }
}
impl ImagesTab {
    pub fn clear(&mut self) {
        self.images.clear();
        self.current_image = None;
        self.current_pull_chunks = None;
        self.central_view = CentralView::None;
    }
}

impl App {
    pub fn link_image(&self, ui: &mut egui::Ui, id: ImageIdRef, name: Option<&str>) {
        if ui
            .add(
                egui::Label::new(
                    name.map(|n| n.trim_start_matches('/'))
                        .unwrap_or_else(|| trim_id(id)),
                )
                .strong()
                .sense(egui::Sense {
                    click: true,
                    focusable: true,
                    drag: false,
                }),
            )
            .on_hover_text("click to follow")
            .clicked()
        {
            let _ = self.send_event(EventRequest::Image(ImageEvent::Inspect {
                id: id.to_string(),
            }));
            let _ = self.send_event(EventRequest::NotifyGui(GuiEvent::SetTab(
                crate::app::Tab::Images,
            )));
        }
    }
    pub fn images_view(&mut self, ui: &mut egui::Ui) {
        match self.images.central_view {
            CentralView::Image => self.image_details(ui),
            CentralView::Pull => self.images_pull(ui),
            CentralView::Search => self.images_search(ui),
            CentralView::None => {}
        }
    }

    pub fn images_side(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            self.images_menu(ui);
            self.images_scroll(ui);
        });
    }

    fn images_menu(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("images_tab_grid").show(ui, |ui| {
            ui.selectable_value(
                &mut self.images.central_view,
                CentralView::None,
                "main view",
            );
            ui.selectable_value(&mut self.images.central_view, CentralView::Pull, "pull");
            ui.selectable_value(&mut self.images.central_view, CentralView::Search, "search");
        });
        egui::Grid::new("images_button_grid").show(ui, |ui| {
            if ui.button("import").clicked() {
                match native_dialog::FileDialog::new()
                    .add_filter("tar archive", &["tar"])
                    .add_filter("tar gzip archive", &["tar.gz", "tgz"])
                    .add_filter("tar bzip2 archive", &["tar.bz2"])
                    .add_filter("tar xz archive", &["tar.xz", "txz"])
                    .show_open_single_file()
                {
                    Ok(Some(path)) => {
                        if let Err(e) =
                            self.send_event(EventRequest::Image(ImageEvent::Import { path }))
                        {
                            self.add_error(e);
                        };
                    }
                    Ok(None) => {}
                    Err(e) => self
                        .add_error(Error::msg(
                            format!("failed to spawn a file dialog - {}", e,),
                        )),
                }
            }
            if ui.button("prune").clicked() {
                self.popups.push_back(ui::ActionPopup::new(
                    EventRequest::Image(ImageEvent::Prune),
                    "Prune images",
                    "Are you sure you want to prune unused images? This will delete all images not in use by a container.",
                ));
            }
        });
    }

    fn images_scroll(&mut self, ui: &mut egui::Ui) {
        let mut view = self.images.central_view;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.wrap_text();
            egui::Grid::new("side_panel")
                .spacing((0., 0.))
                .max_col_width(self.side_panel_size())
                .show(ui, |ui| {
                    let mut errors = vec![];
                    let mut popups = vec![];
                    let color = ui.visuals().widgets.open.bg_fill;
                    for image in &self.images.images {
                        let selected = self
                            .images
                            .current_image
                            .as_ref()
                            .map(|i| {
                                i.details.id == image.id
                                    && self.images.central_view == CentralView::Image
                            })
                            .unwrap_or_default();

                        let frame = if selected {
                            egui::Frame::none().fill(color).margin((0., 0.))
                        } else {
                            egui::Frame::none().margin((0., 0.))
                        };

                        frame.show(ui, |ui| {
                            egui::Grid::new(&image.id).spacing((0., 5.)).show(ui, |ui| {
                                ui::line_with_size(ui, frame, (self.side_panel_size(), 1.));
                                ui.end_row();
                                egui::Grid::new(&image.id[0..8])
                                    .spacing((2.5, 5.))
                                    .max_col_width(self.side_panel_size())
                                    .show(ui, |ui| {
                                        let image_name = name(&image.id, image.repo_tags.as_ref());

                                        ui.add_space(5.);
                                        ui.scope(|ui| {
                                            ui.add(icon());
                                            ui.add(
                                                Label::new(&image_name)
                                                    .heading()
                                                    .strong()
                                                    .wrap(true),
                                            );
                                        });
                                        ui.end_row();

                                        ui.add_space(5.);
                                        ui.add(
                                            Label::new(&image.created.to_rfc2822())
                                                .italics()
                                                .strong()
                                                .wrap(true),
                                        );
                                        ui.end_row();

                                        ui.add_space(5.);
                                        ui.add(
                                            Label::new(crate::conv_b(image.virtual_size))
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
                                                    "display detailed information about the image",
                                                )
                                                .clicked()
                                            {
                                                if let Err(e) = self.send_event(
                                                    EventRequest::Image(ImageEvent::Inspect {
                                                        id: image.id.clone(),
                                                    }),
                                                ) {
                                                    errors.push(e);
                                                };
                                                view = CentralView::Image;
                                            }
                                            if ui
                                                .button(icon::DELETE)
                                                .on_hover_text("delete the image")
                                                .clicked()
                                            {
                                                popups.push(ui::ActionPopup::new(
                                                    EventRequest::Image(ImageEvent::Delete {
                                                        id: image.id.clone(),
                                                    }),
                                                    "Delte image",
                                                    format!(
                                                        "Are you sure you want to delete image {}",
                                                        &image.id
                                                    ),
                                                ));
                                            }
                                            if ui
                                                .button(icon::SAVE)
                                                .on_hover_text(
                                                    "save the image to filesystem as tar archive",
                                                )
                                                .clicked()
                                            {
                                                let tar_name =
                                                    format!("image_{}", trim_id(&image.id));
                                                match native_dialog::FileDialog::new()
                                                    .add_filter("tar archive", &["tar"])
                                                    .set_filename(&tar_name[..])
                                                    .show_save_single_file()
                                                {
                                                    Ok(Some(output_path)) => {
                                                        if let Err(e) = self.send_event(
                                                            EventRequest::Image(ImageEvent::Save {
                                                                id: image.id.clone(),
                                                                output_path,
                                                            }),
                                                        ) {
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
                                        ui.end_row();
                                    });
                                ui.end_row();
                                ui.scope(|_| {});
                                ui.end_row();
                            });
                        });
                        ui.end_row();
                    }
                    errors.iter().for_each(|err| self.add_notification(err));
                    self.popups.extend(popups);
                });
        });
        self.images.central_view = view;
    }

    fn image_details(&self, ui: &mut egui::Ui) {
        if let Some(image) = &self.images.current_image {
            let details = &image.details;

            ui.horizontal(|ui| {
                ui.add(icon());
                ui.add(
                    Label::new(name(&details.id, Some(details.repo_tags.as_ref())))
                        .heading()
                        .wrap(true)
                        .strong(),
                );
            });
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

                key!(ui, "Parent:");
                self.link_image(ui, &details.parent, None);
                ui.end_row();

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

    fn images_pull(&mut self, ui: &mut egui::Ui) {
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
            ui.add(TextEdit::singleline(&mut self.images.pull_view.image).desired_width(150.));
            ui.end_row();
            ui.add(Label::new("User:").strong());
            ui.add(TextEdit::singleline(&mut self.images.pull_view.user).desired_width(150.));
            ui.end_row();
            ui.add(Label::new("Password:").strong());
            ui.add(
                TextEdit::singleline(&mut self.images.pull_view.password)
                    .password(true)
                    .desired_width(150.),
            );
            ui.end_row();
            if ui.button("pull").clicked() {
                if self.images.pull_view.in_progress {
                    self.add_notification("Image pull already in progress");
                } else if self.images.pull_view.image.is_empty() {
                    self.add_notification("Image name can't be empty");
                } else {
                    let auth = if !self.images.pull_view.user.is_empty() {
                        let auth = RegistryAuth::builder();
                        if !self.images.pull_view.password.is_empty() {
                            Some(
                                auth.username(&self.images.pull_view.user)
                                    .password(&self.images.pull_view.password)
                                    .build(),
                            )
                        } else {
                            Some(auth.username(&self.images.pull_view.user).build())
                        }
                    } else {
                        None
                    };
                    self.send_event_notify(EventRequest::Image(ImageEvent::Pull {
                        image: self.images.pull_view.image.clone(),
                        auth,
                    }));
                    self.images.pull_view.in_progress = true;
                    self.images.current_pull_chunks = None;
                }
            }
        });
        let mut text = String::new();
        let mut progress_percent = 0.;
        if let Some(chunks) = self.images.current_pull_chunks.as_ref() {
            for chunk in chunks {
                match chunk {
                    ImageBuildChunk::Update { stream } => {
                        text.push_str("Update: ");
                        text.push_str(stream);
                    }
                    ImageBuildChunk::Error { error, .. } => {
                        text.push_str("Error: ");
                        text.push_str(error);
                    }
                    ImageBuildChunk::Digest { aux } => {
                        text.push_str("Digest: ");
                        text.push_str(&aux.id);
                        progress_percent = 1.;
                    }
                    ImageBuildChunk::PullStatus {
                        status,
                        id: _,
                        progress: _,
                        progress_detail,
                    } => {
                        if status.starts_with("Digest") {
                            progress_percent = 1.;
                        }
                        text.push_str("Status: ");
                        text.push_str(status);
                        if let Some(progress) = progress_detail {
                            if let Some(current) = progress.current {
                                if let Some(total) = progress.total {
                                    progress_percent = current as f32 / total as f32;
                                    text.push_str(&format!(
                                        " ({} / {})",
                                        crate::conv_b(current),
                                        crate::conv_b(total)
                                    ));
                                }
                            }
                        }
                    }
                }
                text.push('\n');
            }
        }
        if self.images.pull_view.in_progress || (progress_percent - 1.).abs() < f32::EPSILON {
            ui.add(
                egui::ProgressBar::new(progress_percent)
                    .desired_width(200.)
                    .animate(true),
            );
        }
        ui.add(
            TextEdit::multiline(&mut text.as_str())
                .code_editor()
                .desired_width(f32::INFINITY),
        );
    }

    fn images_search(&mut self, ui: &mut egui::Ui) {
        ui.add(
            Label::new("Search for images in Docker Hub")
                .heading()
                .strong(),
        );
        ui.add_space(25.);

        key!(ui, "Term:");
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut self.images.search_view.image).desired_width(150.));
            if ui.button("search").clicked() {
                if self.images.search_view.image.is_empty() {
                    self.add_error(
                        "Can't search for an empty term. Enter a name of the image to search for",
                    );
                } else {
                    self.send_event_notify(EventRequest::Image(ImageEvent::Search {
                        image: self.images.search_view.image.clone(),
                    }))
                }
            }
        });

        ui.add_space(15.);

        let mut progress_percent = 0.;
        if let Some(chunks) = self.images.current_pull_chunks.as_ref() {
            for chunk in chunks {
                match chunk {
                    ImageBuildChunk::Digest { aux: _ } => {
                        progress_percent = 1.;
                    }
                    ImageBuildChunk::PullStatus {
                        status,
                        id: _,
                        progress: _,
                        progress_detail,
                    } => {
                        if status.starts_with("Digest") {
                            progress_percent = 1.;
                        }
                        if let Some(progress) = progress_detail {
                            if let Some(current) = progress.current {
                                if let Some(total) = progress.total {
                                    progress_percent = current as f32 / total as f32;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if self.images.search_view.pull_in_progress || (progress_percent - 1.).abs() < f32::EPSILON
        {
            ui.add(
                egui::ProgressBar::new(progress_percent)
                    .desired_width(200.)
                    .animate(true),
            );
        }

        let mut pull_events = vec![];

        if let Some(images) = self.images.search_view.images.as_ref() {
            Grid::new("images_search_grid")
                .striped(true)
                .spacing((10., 10.))
                .show(ui, |ui| {
                    ui.scope(|_| {});
                    key!(ui, "Name");
                    key!(ui, "Star count");
                    key!(ui, "Official");
                    key!(ui, "Automated");
                    ui.scope(|ui| {
                        key!(ui, "Description");
                        ui.allocate_space((ui.available_width() / 5., 0.).into());
                    });
                    ui.end_row();

                    for image in images {
                        if ui.button(icon::ARROW_DOWN).clicked() {
                            // #TODO: handle auth
                            pull_events.push(EventRequest::Image(ImageEvent::Pull {
                                image: image.name.clone(),
                                auth: None,
                            }));
                            self.images.current_pull_chunks = None;
                            self.images.search_view.pull_in_progress = true;
                        }
                        ui.scope(|ui| {
                            ui.add(icon());
                            val!(ui, &image.name);
                        });
                        val!(ui, image.star_count);
                        ui.add(ui::bool_icon(image.is_official));
                        ui.add(ui::bool_icon(image.is_automated));
                        val!(ui, &image.description);
                        ui.end_row();
                    }
                });
        }
        for event in pull_events {
            self.send_event_notify(event);
        }
    }
}
