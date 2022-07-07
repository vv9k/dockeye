use egui::{FontDefinitions, FontFamily, TextStyle};
use serde::{Deserialize, Serialize};

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

impl FontSizes {}
