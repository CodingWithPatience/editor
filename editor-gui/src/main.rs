#![windows_subsystem = "windows"]

mod app;
mod gui_renderer;
mod keymap;

use crate::app::EditorApp;

fn main() -> Result<(), eframe::Error> {
    let filename = std::env::args().nth(1);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Editor GUI",
        options,
        Box::new(move |_cc| Ok(Box::new(EditorApp::new(filename)))),
    )
}
