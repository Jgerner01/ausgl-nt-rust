#![cfg_attr(not(test), windows_subsystem = "windows")]
#![allow(dead_code)]

mod adjust;
mod app;
mod approx;
mod caplan;
mod export;
mod geo;
mod io;
mod linalg;
mod netfile;
mod pdf;
mod report;
mod rtf;
mod sample;
mod scene;
mod tools;
mod update;
mod version;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" if i + 1 < args.len() => {
                screenshot = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            a if !a.starts_with("--") => open = Some(PathBuf::from(a)),
            _ => {}
        }
        i += 1;
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("AUSGL-NT – Geodätische Netzausgleichung")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([900.0, 560.0]);
    // Fenstersymbol (dasselbe Motiv wie das EXE-Icon)
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/ausgl-nt.png")) {
        viewport = viewport.with_icon(icon);
    }
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("AUSGL-NT", options, Box::new(move |cc| Ok(Box::new(app::AusglApp::new(cc, screenshot, open)))))
}
