mod desktop_app;

use anyhow::Result;
use desktop_app::ViewDesktopApp;
use eframe::{egui, Renderer};

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("VIEW Desktop")
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: Renderer::Glow,
        ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "VIEW Desktop",
        options,
        Box::new(|cc| Box::new(ViewDesktopApp::new(cc))),
    )
    .map_err(|error| anyhow::anyhow!("Failed to launch VIEW Desktop: {error}"))
}
