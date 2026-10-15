//! table-plus-rs: a fast, native PostgreSQL client. Pure Rust (egui), no webview.
//!
//! Environment:
//! - `TABLE_PLUS_RS_HOME`  config directory override (connections, history, settings)
//! - `TABLE_PLUS_RS_THEME` theme id (see Settings); `--theme NAME` on the command line wins
//! - `TABLE_PLUS_RS_RENDERER=glow|wgpu` force a renderer (default: wgpu on macOS)
//! - `RUST_LOG` log filter

mod app;
mod editor;
mod grid;
mod process_tab;
mod query_tab;
mod script;
mod sidebar;
mod structure;
mod table_tab;
mod task;
mod theme;
mod toolbar;
mod welcome;
mod widgets;
mod workspace;

fn data_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("TABLE_PLUS_RS_HOME") {
        return d.into();
    }
    // Same folder the app has always used, so saved connections carry over.
    dirs::data_dir().unwrap_or_else(|| ".".into()).join("dev.zeeshan.table-plus-rs")
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,table_plus_core=info")).init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("table-plus-io")
        .enable_all()
        .build()
        .expect("tokio runtime");

    let renderer = match std::env::var("TABLE_PLUS_RS_RENDERER").as_deref() {
        Ok("wgpu") => eframe::Renderer::Wgpu,
        Ok("glow") => eframe::Renderer::Glow,
        _ if cfg!(target_os = "macos") => eframe::Renderer::Wgpu,
        _ => eframe::Renderer::Glow,
    };

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("table-plus-rs")
        .with_app_id("table-plus-rs")
        .with_inner_size([1360.0, 860.0])
        .with_min_inner_size([860.0, 520.0]);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    if std::env::var_os("TABLE_PLUS_RS_SHOTS").is_some() {
        // Scripted runs: macOS pauses redraws for covered windows, so stay on top.
        viewport = viewport.with_always_on_top().with_position([40.0, 40.0]);
    }
    let options = eframe::NativeOptions { viewport, renderer, centered: true, ..Default::default() };
    let dir = data_dir();
    eframe::run_native("table-plus-rs", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, runtime, dir)))))
}
