use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const FILENAME: &str = "dockeye.yml";

pub fn dir() -> Option<PathBuf> {
    dirs::config_dir()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Settings {
    pub docker_addr: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            docker_addr: crate::DEFAULT_DOCKER_ADDR.to_string(),
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
                ui.label("Docker address:");
                ui.text_edit_singleline(&mut self.settings.docker_addr);
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
        self.show = show;
        self.msg = msg;
    }
}
