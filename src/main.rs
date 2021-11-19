fn main() {
    let app = dockeye::App::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}
