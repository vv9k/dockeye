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

impl FontSizes {
    fn fonts_differ(&self, current_fonts: &FontDefinitions) -> bool {
        for (style, (_, size)) in &current_fonts.family_and_size {
            let size = *size;
            match style {
                TextStyle::Small => {
                    if (self.small - size).abs() > f32::EPSILON {
                        return true;
                    }
                }
                TextStyle::Body => {
                    if (self.body - size).abs() > f32::EPSILON {
                        return true;
                    }
                }
                TextStyle::Button => {
                    if (self.button - size).abs() > f32::EPSILON {
                        return true;
                    }
                }
                TextStyle::Heading => {
                    if (self.heading - size).abs() > f32::EPSILON {
                        return true;
                    }
                }
                TextStyle::Monospace => {
                    if (self.monospace - size).abs() > f32::EPSILON {
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
