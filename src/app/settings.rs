use anyhow::{Context, Result};
use egui::{FontDefinitions, FontFamily, TextStyle};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const FILENAME: &str = "dockeye.yml";
const ALLOWED_FONT_SIZE: std::ops::RangeInclusive<f32> = 10.0..=50.0;

pub fn dir() -> Option<PathBuf> {
    dirs::config_dir()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FontSizes {
    pub small: f32,
    pub body: f32,
    pub button: f32,
    pub heading: f32,
    pub monospace: f32,
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            small: 10.,
            body: 14.,
            button: 14.,
            heading: 20.,
            monospace: 14.,
        }
    }
}

impl FontSizes {
    fn fonts_differ(&self, current_fonts: &FontDefinitions) -> bool {
        for (style, (_, size)) in &current_fonts.family_and_size {
            let size = *size;
            match style {
                TextStyle::Small => {
                    if self.small != size {
                        return true;
                    }
                }
                TextStyle::Body => {
                    if self.body != size {
                        return true;
                    }
                }
                TextStyle::Button => {
                    if self.button != size {
                        return true;
                    }
                }
                TextStyle::Heading => {
                    if self.heading != size {
                        return true;
                    }
                }
                TextStyle::Monospace => {
                    if self.monospace != size {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn update_ctx(&self, ctx: &egui::CtxRef) {
        let current_fonts = ctx.fonts();

        if !self.fonts_differ(current_fonts.definitions()) {
            return;
        }

        let mut fonts = FontDefinitions::default();

        fonts
            .family_and_size
            .insert(TextStyle::Small, (FontFamily::Proportional, self.small));
        fonts
            .family_and_size
            .insert(TextStyle::Small, (FontFamily::Monospace, self.small));

        fonts
            .family_and_size
            .insert(TextStyle::Body, (FontFamily::Proportional, self.body));
        fonts
            .family_and_size
            .insert(TextStyle::Body, (FontFamily::Monospace, self.body));

        fonts
            .family_and_size
            .insert(TextStyle::Button, (FontFamily::Proportional, self.button));
        fonts
            .family_and_size
            .insert(TextStyle::Button, (FontFamily::Monospace, self.button));

        fonts
            .family_and_size
            .insert(TextStyle::Heading, (FontFamily::Proportional, self.heading));
        fonts
            .family_and_size
            .insert(TextStyle::Heading, (FontFamily::Monospace, self.heading));

        fonts.family_and_size.insert(
            TextStyle::Monospace,
            (FontFamily::Proportional, self.monospace),
        );
        fonts.family_and_size.insert(
            TextStyle::Monospace,
            (FontFamily::Monospace, self.monospace),
        );

        ctx.set_fonts(fonts);
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub docker_addr: String,
    pub fonts: FontSizes,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            docker_addr: crate::DEFAULT_DOCKER_ADDR.to_string(),
            fonts: FontSizes::default(),
        }
    }
}

impl Settings {
    /// Loads the settings from the configuration file located at `path`. The configuration file is
    /// expected to be a valid YAML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let data = fs::read(path).context("failed to read configuration file")?;
        serde_yaml::from_slice(&data).context("failed to deserialize configuration")
    }

    /// Saves this settings as YAML file in the provided `path`.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let data = serde_yaml::to_vec(&self).context("failed to serialize settings")?;
        fs::write(path, &data).context("failed to write settings to file")
    }
}

#[derive(Debug)]
pub enum Message {
    Error(String),
    Ok(String),
}

#[derive(Debug)]
pub struct SettingsWindow {
    pub show: bool,
    pub settings: Settings,
    pub settings_path: Option<PathBuf>,
    pub msg: Option<Message>,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            show: false,
            settings: Settings::default(),
            settings_path: dir().map(|d| d.join(FILENAME)),
            msg: None,
        }
    }
}

impl SettingsWindow {
    pub fn toggle(&mut self) {
        self.show = !self.show;
    }

    pub fn save_settings(&mut self) -> Result<()> {
        if let Some(settings_path) = &self.settings_path {
            log::trace!("saving settings");
            self.settings.save(&settings_path)
        } else {
            Ok(())
        }
    }

    pub fn display(&mut self, ctx: &egui::CtxRef) {
        let mut show = self.show;
        let mut msg = std::mem::take(&mut self.msg);
        egui::Window::new("settings")
            .open(&mut show)
            .show(ctx, |ui| {
                if let Some(m) = &msg {
                    let (color, m) = match m {
                        Message::Ok(m) => (egui::Color32::GREEN, m),
                        Message::Error(m) => (egui::Color32::RED, m),
                    };
                    ui.add(egui::Label::new(m).text_color(color));
                }
                egui::Grid::new("settings_grid").show(ui, |ui| {
                    ui.label("Docker address:");
                    ui.text_edit_singleline(&mut self.settings.docker_addr)
                        .on_hover_text(
                            r#"Can be one of:
 - unix:///path/to/docker.sock
 - http://some.http.con.com
 - https://some.https.con.com
"#,
                        );
                    ui.end_row();

                    self.fonts_ui(ui);
                    ui.end_row();

                    if ui.button("save").clicked() {
                        if let Err(e) = self.save_settings() {
                            msg = Some(Message::Error(format!("{:?}", e)));
                        } else {
                            msg = Some(Message::Ok(format!(
                                "successfully saved settings {}",
                                self.settings_path
                                    .as_deref()
                                    .map(|p| format!("to {}", p.display()))
                                    .unwrap_or_default(),
                            )));
                        }
                    }
                });
            });
        self.show = show;
        self.msg = msg;
    }
    fn fonts_ui(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("fonts")
            .default_open(false)
            .show(ui, |ui| {
                egui::Grid::new("fonts_grid").show(ui, |ui| {
                    ui.label("small");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.fonts.small)
                            .clamp_range(ALLOWED_FONT_SIZE),
                    );
                    ui.end_row();
                    ui.label("body");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.fonts.body)
                            .clamp_range(ALLOWED_FONT_SIZE),
                    );
                    ui.end_row();
                    ui.label("button");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.fonts.button)
                            .clamp_range(ALLOWED_FONT_SIZE),
                    );
                    ui.end_row();
                    ui.label("heading");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.fonts.heading)
                            .clamp_range(ALLOWED_FONT_SIZE),
                    );
                    ui.end_row();
                    ui.label("monospace");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.fonts.monospace)
                            .clamp_range(ALLOWED_FONT_SIZE),
                    );
                    ui.end_row();
                });
            });
    }
}
