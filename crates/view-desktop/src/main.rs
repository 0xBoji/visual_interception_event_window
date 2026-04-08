mod desktop_app;

use anyhow::Result;
use desktop_app::ViewDesktopApp;
use eframe::{egui, Renderer};
use std::fs::OpenOptions;
use std::io::Write;

fn main() -> Result<()> {
    main_debug_log("desktop main: starting".to_string());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("VIEW Desktop")
            .with_inner_size([1600.0, 980.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: Renderer::Glow,
        ..eframe::NativeOptions::default()
    };

    main_debug_log("desktop main: calling run_native".to_string());
    let result = eframe::run_native(
        "VIEW Desktop",
        options,
        Box::new(|cc| {
            main_debug_log("desktop main: app creator invoked".to_string());
            Ok(Box::new(ViewDesktopApp::new(cc)))
        }),
    );
    main_debug_log(format!(
        "desktop main: run_native returned {:?}",
        result.as_ref().err()
    ));
    result.map_err(|error| anyhow::anyhow!("Failed to launch VIEW Desktop: {error}"))
}

fn main_debug_log(message: String) {
    let Ok(path) = std::env::var("VIEW_DESKTOP_DEBUG_LOG") else {
        return;
    };

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{message}");
    }
}
