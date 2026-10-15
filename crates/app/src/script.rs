//! Dev automation: drives the real UI through a scenario and saves screenshots with egui's own
//! capture (no OS screen-recording permission needed). Used for the README screenshots and as a
//! smoke test of the whole stack against a live database.
//!
//!   TABLE_PLUS_RS_HOME=/tmp/tpr-demo \
//!   TABLE_PLUS_RS_SHOTS=docs/screenshots \
//!   TABLE_PLUS_RS_SHOT_DB=127.0.0.1:54329/table_plus_rs_demo \
//!   cargo run -p table-plus-rs

use std::path::PathBuf;

use eframe::egui::{self, Event, ViewportCommand};
use table_plus_core::model::ConnectionConfig;

use crate::app::App;

enum Step {
    Seed,
    Wait(u32),
    Shot(&'static str),
    Connect,
    WaitWorkspace,
    Open(&'static str),
    WaitIdle,
    View(&'static str),
    Query(&'static str),
    RunQuery,
    Theme(&'static str),
    Settings,
    CloseSettings,
    Edit(usize, usize, &'static str),
    Preview,
    CloseDialog,
    Log(&'static str),
    Quit,
}

pub struct Script {
    dir: PathBuf,
    db: String,
    steps: Vec<Step>,
    i: usize,
    frames: u32,
    waiting_shot: bool,
}

impl Script {
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var("TABLE_PLUS_RS_SHOTS").ok()?;
        let db = std::env::var("TABLE_PLUS_RS_SHOT_DB").unwrap_or_else(|_| "127.0.0.1:5432/postgres".into());
        let steps = vec![
            Step::Seed,
            Step::Wait(6),
            Step::Shot("welcome"),
            Step::Theme("one-dark"),
            Step::Wait(8),
            Step::Shot("welcome-one-dark"),
            Step::Theme("default"),
            Step::Wait(4),
            Step::Connect,
            Step::WaitWorkspace,
            Step::Log("connected"),
            Step::Open("users"),
            Step::WaitIdle,
            Step::Shot("table-data"),
            Step::Edit(1, 1, "Renamed in the grid"),
            Step::Wait(4),
            Step::Shot("table-editing"),
            Step::Preview,
            Step::Wait(20),
            Step::Shot("commit-preview"),
            Step::CloseDialog,
            Step::View("structure"),
            Step::WaitIdle,
            Step::Shot("structure"),
            Step::View("index"),
            Step::Wait(4),
            Step::Shot("indexes"),
            Step::Query("SELECT u.id, u.name, count(p.*) AS posts, max(p.score) AS best\nFROM users u\nJOIN posts p ON p.user_id = u.id\nWHERE u.active\nGROUP BY u.id, u.name\nORDER BY posts DESC, u.id\nLIMIT 100;"),
            Step::RunQuery,
            Step::WaitIdle,
            Step::Shot("sql-results"),
            Step::Query("SELECT pg_sleep(4), now();"),
            Step::RunQuery,
            Step::Wait(30),
            Step::Shot("loading"),
            Step::WaitIdle,
            Step::Theme("arc-blueberry"),
            Step::Wait(6),
            Step::Shot("theme-arc-blueberry"),
            Step::Theme("atelier"),
            Step::Settings,
            Step::Wait(20),
            Step::Shot("settings"),
            Step::CloseSettings,
            Step::Log("done"),
            Step::Quit,
        ];
        Some(Self { dir: dir.into(), db, steps, i: 0, frames: 0, waiting_shot: false })
    }

    pub fn step(&mut self, ctx: &egui::Context, app: &mut App) {
        let Some(step) = self.steps.get(self.i) else { return };
        ctx.request_repaint();
        if self.i == 0 && self.frames == 0 {
            eprintln!("[script] first frame, running {} steps", self.steps.len());
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        self.frames += 1;
        let mut advance = true;
        match step {
            Step::Seed => {
                let (hostport, dbname) = self.db.split_once('/').unwrap_or((&self.db, "postgres"));
                let (host, port) = hostport.split_once(':').unwrap_or((hostport, "5432"));
                let mut cfg = ConnectionConfig::blank();
                cfg.name = "demo".into();
                cfg.host = host.into();
                cfg.port = port.parse().unwrap_or(5432);
                cfg.database = dbname.into();
                cfg.ssl_mode = "disable".into();
                cfg.environment = "Development".into();
                cfg.color = "1".into();
                let env = app.script_env();
                if env.storage.list_connections().map(|(c, _)| c.iter().all(|c| c.name != "demo")).unwrap_or(true) {
                    let _ = env.storage.save_connection(cfg);
                }
                app.script_reload_welcome();
            }
            Step::Wait(n) => advance = self.frames >= *n,
            Step::Shot(name) => {
                if !self.waiting_shot {
                    ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
                    self.waiting_shot = true;
                    advance = false;
                } else {
                    let img = ctx.input(|i| {
                        i.raw.events.iter().find_map(|e| match e {
                            Event::Screenshot { image, .. } => Some(image.clone()),
                            _ => None,
                        })
                    });
                    match img {
                        Some(image) => {
                            let _ = std::fs::create_dir_all(&self.dir);
                            let path = self.dir.join(format!("{name}.png"));
                            match save_png(&path, &image) {
                                Ok(()) => eprintln!("[script] saved {}", path.display()),
                                Err(e) => eprintln!("[script] could not save {}: {e}", path.display()),
                            }
                            self.waiting_shot = false;
                        }
                        None => advance = false,
                    }
                }
            }
            Step::Connect => {
                if !app.script_connect("demo") {
                    eprintln!("[script] no connection named demo");
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Step::WaitWorkspace => {
                advance = match app.script_workspace() {
                    Some(ws) => !ws.is_loading_listing(),
                    None => false,
                };
                if self.frames > 600 {
                    eprintln!("[script] timed out connecting");
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Step::Open(name) => {
                let name = name.to_string();
                let env_ptr: *mut _ = app.script_env();
                let _ = env_ptr;
                if let Some(ws) = app.script_workspace() {
                    ws.script_open(&name);
                }
            }
            Step::WaitIdle => {
                advance = self.frames > 5 && app.script_workspace().map(|w| !w.active_busy() && !w.is_loading_listing()).unwrap_or(false);
                if self.frames > 600 {
                    eprintln!("[script] timed out waiting for idle");
                    advance = true;
                }
            }
            Step::View(v) => {
                let v = v.to_string();
                app.script_with_workspace(|ws, env| ws.script_view(env, &v));
            }
            Step::Query(sql) => {
                let sql = sql.to_string();
                app.script_with_workspace(|ws, _| ws.script_query(&sql));
            }
            Step::RunQuery => app.script_with_workspace(|ws, env| ws.script_run(env)),
            Step::Theme(t) => app.script_set_theme(ctx, t),
            Step::Settings => app.script_open_settings(),
            Step::CloseSettings => app.script_close_settings(ctx),
            Step::Edit(r, c, t) => {
                let (r, c, t) = (*r, *c, t.to_string());
                app.script_with_workspace(|ws, _| ws.script_edit(r, c, &t));
            }
            Step::Preview => app.script_with_workspace(|ws, env| ws.script_preview(env)),
            Step::CloseDialog => app.script_with_workspace(|ws, _| ws.script_discard()),
            Step::Log(s) => eprintln!("[script] {s}"),
            Step::Quit => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
                advance = false;
            }
        }
        if advance {
            self.i += 1;
            self.frames = 0;
        }
    }
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), image.size[0] as u32, image.size[1] as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header()?;
    let mut data = Vec::with_capacity(image.pixels.len() * 4);
    for p in &image.pixels {
        data.extend_from_slice(&p.to_array());
    }
    w.write_image_data(&data)?;
    Ok(())
}
