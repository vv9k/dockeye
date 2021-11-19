use eframe::{egui, epi};

pub struct App {}

impl Default for App {
    fn default() -> Self {
        Self {}
    }
}

impl epi::App for App {
    fn name(&self) -> &str {
        "dockeye"
    }

    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>,
    ) {
    }

    #[cfg(feature = "persistence")]
    fn save(&mut self, _storage: &mut dyn epi::Storage) {}

    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {});

        egui::SidePanel::left("side_panel").show(ctx, |ui| {});

        egui::CentralPanel::default().show(ctx, |ui| {});
    }
}
