mod editable_list;
mod popup;

use egui::{
    style::{Selection, Widgets},
    Color32, Label, RichText, Stroke, Visuals, Widget,
};
use epaint::Shadow;
use std::string::ToString;

pub use editable_list::{EditableList, EditableListBuilder};
pub use popup::{ActionPopup, Popup};

pub mod color {
    use egui::{Color32, Rgba};
    use lazy_static::lazy_static;

    lazy_static! {
        pub static ref D_BG_000: Color32 = Color32::from_rgb(0x0e, 0x12, 0x17);
        pub static ref D_BG_00: Color32 = Color32::from_rgb(0x11, 0x16, 0x1b);
        pub static ref D_BG_0: Color32 = Color32::from_rgb(0x16, 0x1c, 0x23);
        pub static ref D_BG_1: Color32 = Color32::from_rgb(0x23, 0x2d, 0x38);
        pub static ref D_BG_2: Color32 = Color32::from_rgb(0x31, 0x3f, 0x4e);
        pub static ref D_BG_3: Color32 = Color32::from_rgb(0x41, 0x53, 0x67);
        pub static ref D_FG_0: Color32 = Color32::from_rgb(0xe5, 0xde, 0xd6);
        pub static ref D_FG_1: Color32 = Color32::from_rgb(0xc4, 0xbe, 0xb7);
        pub static ref D_BG_00_TRANSPARENT: Color32 = Rgba::from(*D_BG_00).multiply(0.96).into();
        pub static ref D_BG_0_TRANSPARENT: Color32 = Rgba::from(*D_BG_0).multiply(0.96).into();
        pub static ref D_BG_1_TRANSPARENT: Color32 = Rgba::from(*D_BG_1).multiply(0.96).into();
        pub static ref D_BG_2_TRANSPARENT: Color32 = Rgba::from(*D_BG_2).multiply(0.96).into();
        pub static ref D_BG_3_TRANSPARENT: Color32 = Rgba::from(*D_BG_3).multiply(0.96).into();
        pub static ref L_BG_0: Color32 = Color32::from_rgb(0xbf, 0xbf, 0xbf);
        pub static ref L_BG_1: Color32 = Color32::from_rgb(0xd4, 0xd3, 0xd4);
        pub static ref L_BG_2: Color32 = Color32::from_rgb(0xd9, 0xd9, 0xd9);
        pub static ref L_BG_3: Color32 = Color32::from_rgb(0xea, 0xea, 0xea);
        pub static ref L_BG_4: Color32 = Color32::from_rgb(0xf9, 0xf9, 0xf9);
        pub static ref L_BG_5: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
        pub static ref L_BG_0_TRANSPARENT: Color32 = Rgba::from(*L_BG_0).multiply(0.86).into();
        pub static ref L_BG_1_TRANSPARENT: Color32 = Rgba::from(*L_BG_1).multiply(0.86).into();
        pub static ref L_BG_2_TRANSPARENT: Color32 = Rgba::from(*L_BG_2).multiply(0.86).into();
        pub static ref L_BG_3_TRANSPARENT: Color32 = Rgba::from(*L_BG_3).multiply(0.86).into();
        pub static ref L_BG_4_TRANSPARENT: Color32 = Rgba::from(*L_BG_4).multiply(0.86).into();
        pub static ref L_BG_5_TRANSPARENT: Color32 = Rgba::from(*L_BG_5).multiply(0.86).into();
        pub static ref L_FG_0: Color32 = Color32::from_rgb(0x08, 0x08, 0x08);
        pub static ref L_FG_1: Color32 = Color32::from_rgb(0x0c, 0x0c, 0x0c);
    }
}

pub mod icon {
    pub const PACKAGE: &str = "\u{1F4E6}";
    pub const SCROLL: &str = "\u{1F4DC}";
    pub const INFO: &str = "\u{2139}";
    pub const DELETE: &str = "\u{1F5D9}";
    pub const PLAY: &str = "\u{25B6}";
    pub const PAUSE: &str = "\u{23F8}";
    pub const STOP: &str = "\u{23F9}";
    pub const SETTINGS: &str = "\u{2699}";
    pub const SAVE: &str = "\u{1F4BE}";
    pub const ADD: &str = "\u{2795}";
    pub const SUB: &str = "\u{2796}";
    pub const DISK: &str = "\u{1F5B4}";
    pub const ARROW_DOWN: &str = "\u{2B8B}";
    pub const ARROW_LEFT: &str = "\u{2B05}";
    pub const ARROW_RIGHT: &str = "\u{27A1}";
    pub const RESTART: &str = "\u{1F504}";
    pub const NETWORK: &str = "\u{1F5A7}";
}

pub fn light_visuals() -> Visuals {
    use color::*;
    let mut widgets = Widgets::light();
    widgets.noninteractive.bg_fill = *L_BG_2_TRANSPARENT;
    widgets.inactive.bg_fill = *L_BG_2_TRANSPARENT;
    widgets.hovered.bg_fill = *L_BG_3_TRANSPARENT;
    widgets.open.bg_fill = *L_BG_3_TRANSPARENT;
    widgets.active.bg_fill = *L_BG_4_TRANSPARENT;

    widgets.noninteractive.fg_stroke = Stroke::new(1.2, *L_FG_1);
    widgets.inactive.fg_stroke = Stroke::new(1.2, *L_FG_1);
    widgets.hovered.fg_stroke = Stroke::new(1.5, *L_FG_1);
    widgets.open.fg_stroke = Stroke::new(1.5, *L_FG_1);
    widgets.active.fg_stroke = Stroke::new(1.5, *L_FG_0);

    Visuals {
        dark_mode: false,
        extreme_bg_color: Color32::WHITE,
        selection: Selection {
            bg_fill: *L_BG_5,
            stroke: Stroke::new(0.7, *D_BG_0),
        },
        popup_shadow: Shadow::small_light(),
        widgets,
        faint_bg_color: *L_BG_0,
        ..Default::default()
    }
}

pub fn dark_visuals() -> Visuals {
    use color::*;
    let mut widgets = Widgets::dark();
    widgets.noninteractive.bg_fill = *D_BG_0_TRANSPARENT;
    widgets.inactive.bg_fill = *D_BG_1_TRANSPARENT;
    widgets.hovered.bg_fill = *D_BG_2_TRANSPARENT;
    widgets.open.bg_fill = *D_BG_2_TRANSPARENT;
    widgets.active.bg_fill = *D_BG_3_TRANSPARENT;

    widgets.noninteractive.fg_stroke = Stroke::new(0.7, *D_FG_1);
    widgets.inactive.fg_stroke = Stroke::new(0.7, *D_FG_1);
    widgets.hovered.fg_stroke = Stroke::new(1., *D_FG_0);
    widgets.open.fg_stroke = Stroke::new(1., *D_FG_0);
    widgets.active.fg_stroke = Stroke::new(1.5, *D_FG_0);

    Visuals {
        dark_mode: true,
        extreme_bg_color: Color32::BLACK,
        selection: Selection {
            bg_fill: *D_BG_3_TRANSPARENT,
            stroke: Stroke::new(0.7, *D_FG_0),
        },
        popup_shadow: Shadow::small_dark(),
        widgets,
        faint_bg_color: *D_BG_00,
        ..Default::default()
    }
}

#[macro_export]
macro_rules! key {
    ($ui:ident, $k:expr) => {
        $ui.add(crate::app::ui::key_label($k));
    };
}

#[macro_export]
macro_rules! val {
    ($ui:ident, $v:expr) => {
        $ui.add(crate::app::ui::copyable_label($v));
    };
}

#[macro_export]
macro_rules! key_val {
    ($ui:ident, $k:expr, $v:expr) => {
        crate::app::ui::key!($ui, $k);
        crate::app::ui::val!($ui, $v);
        $ui.end_row();
    };
}

pub use key;
pub use key_val;
pub use val;

pub fn key_label(label: impl ToString) -> impl Widget + 'static {
    let label = label.to_string();
    move |ui: &mut egui::Ui| {
        let text = RichText::new(label).strong();
        ui.add(egui::Label::new(text).sense(egui::Sense {
            click: true,
            focusable: true,
            drag: false,
        }))
    }
}

pub fn copyable_label(label: impl ToString) -> impl Widget + 'static {
    let label = label.to_string();
    move |ui: &mut egui::Ui| {
        let text = RichText::new(&label).monospace();
        let rsp = ui
            .add(egui::Label::new(text).sense(egui::Sense {
                click: true,
                focusable: true,
                drag: false,
            }))
            .on_hover_text("secondary-click to copy");
        if rsp.secondary_clicked() {
            if let Err(e) = crate::save_to_clipboard(label) {
                log::error!("failed to save content to clipboard - {}", e);
            }
        }
        rsp
    }
}

#[allow(dead_code)]
pub fn line(frame: egui::Frame) -> impl Widget + 'static {
    move |ui: &mut egui::Ui| line_with_size(frame, ui.available_size()).ui(ui)
}

pub fn line_with_size(frame: egui::Frame, size: impl Into<egui::Vec2>) -> impl Widget + 'static {
    let size = size.into();
    move |ui: &mut egui::Ui| {
        frame.show(ui, |ui| {
            ui.set_max_height(1.);

            let size = egui::vec2(size.x, 0.);

            let (rect, response) = ui.allocate_at_least(size, egui::Sense::hover());
            let points = [
                egui::pos2(rect.left(), rect.bottom()),
                egui::pos2(rect.right(), rect.bottom()),
            ];

            let stroke = ui.visuals().widgets.noninteractive.fg_stroke;
            ui.painter().line_segment(points, stroke);
            response
        })
    }.response
}

pub fn bool_icon(val: bool) -> Label {
    let (icon, color) = if val {
        (icon::ADD, Color32::GREEN)
    } else {
        (icon::SUB, Color32::RED)
    };
    Label::new(RichText::new(icon).strong().color(color))
}
