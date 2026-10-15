//! Top-level application: owns storage, theme, the welcome screen, the open workspace and toasts.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use eframe::egui::{self, Align2, CornerRadius, RichText, Stroke, Ui};
use table_plus_core::model::{AppSettings, ConnectionConfig};
use table_plus_core::storage::Storage;
use table_plus_core::{AppError, Session};

use crate::task::{poll_slot, Spawner, Task};
use crate::theme::{self, c32, Colors};
use crate::welcome::{self, Welcome};
use crate::widgets;
use crate::workspace::{self, Workspace};

pub fn err_msg(e: AppError) -> anyhow::Error {
    anyhow!(e.payload().one_line())
}

/// Shared environment handed to every screen.
pub struct Env {
    pub sp: Spawner,
    pub storage: Arc<Storage>,
    pub colors: Arc<Colors>,
    pub settings: AppSettings,
    pub toasts: Vec<Toast>,
}

pub struct Toast {
    pub text: String,
    pub ok: bool,
    born: Instant,
}

impl Env {
    pub fn ok(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), ok: true, born: Instant::now() });
    }

    pub fn err(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), ok: false, born: Instant::now() });
    }

    pub fn save_settings(&mut self) {
        if let Err(e) = self.storage.save_settings(&self.settings) {
            self.err(format!("Could not save settings: {}", e.payload().one_line()));
        }
    }
}

struct Connecting {
    name: String,
    task: Task<Arc<Session>>,
}

struct SettingsDialog {
    original: usize,
    picked: usize,
    settings: AppSettings,
}

pub struct App {
    _rt: tokio::runtime::Runtime,
    env: Env,
    theme_index: usize,
    welcome: Welcome,
    ws: Option<Box<Workspace>>,
    connecting: Option<Connecting>,
    settings_dialog: Option<SettingsDialog>,
    pub script: Option<crate::script::Script>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime, dir: std::path::PathBuf) -> Self {
        let storage = Arc::new(Storage::new(dir).expect("storage directory"));
        let settings = storage.settings().unwrap_or_default();
        let sp = Spawner::new(rt.handle().clone(), cc.egui_ctx.clone());
        let mut env = Env { sp, storage, colors: Arc::new(theme::jade()), settings, toasts: vec![] };

        let flag = theme::flag_from_args(std::env::args().skip(1));
        let env_var = std::env::var("TABLE_PLUS_RS_THEME").ok();
        let theme_index = match theme::resolve(flag.as_deref(), env_var.as_deref(), Some(&env.settings.theme)) {
            Ok(choice) => {
                if let Some(w) = choice.warning {
                    env.err(w);
                }
                choice.index
            }
            Err(e) => {
                env.err(e);
                theme::find(&env.settings.theme).unwrap_or(0)
            }
        };
        env.colors = Arc::new(theme::colors_for(theme_index));
        theme::apply(&cc.egui_ctx, &env.colors);

        let welcome = Welcome::new(&env);
        let script = crate::script::Script::from_env();
        Self { _rt: rt, env, theme_index, welcome, ws: None, connecting: None, settings_dialog: None, script }
    }

    fn set_theme(&mut self, ctx: &egui::Context, index: usize) {
        self.theme_index = index;
        self.env.colors = Arc::new(theme::colors_for(index));
        theme::apply(ctx, &self.env.colors);
    }

    pub fn start_connect(&mut self, cfg: ConnectionConfig, database: Option<String>) {
        let storage = self.env.storage.clone();
        self.welcome.error = None;
        self.welcome.connecting = Some(cfg.id.clone());
        self.env.settings.last_connection_id = Some(cfg.id.clone());
        self.env.save_settings();
        let name = cfg.name.clone();
        let task = self.env.sp.spawn(async move { Session::connect(cfg, database, Some(storage)).await.map_err(err_msg) });
        self.connecting = Some(Connecting { name, task });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(c) = &mut self.connecting {
            if let Some(res) = c.task.poll() {
                let name = c.name.clone();
                self.connecting = None;
                self.welcome.connecting = None;
                match res {
                    Ok(session) => {
                        self.env.ok(format!("Connected to {name}"));
                        self.ws = Some(Box::new(Workspace::new(&mut self.env, session)));
                    }
                    Err(e) => {
                        self.welcome.error = Some(format!("{name}: {e}"));
                        self.env.err(format!("{name}: {e}"));
                    }
                }
            }
        }
        let _ = ctx;
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.settings_dialog else { return };
        let colors = self.env.colors.clone();
        let (mut save, mut cancel) = (false, false);
        let mut picked = d.picked;
        let modal = widgets::modal(ctx, &colors, "settings").show(ctx, |ui| {
            ui.set_width(480.0);
            ui.heading("Settings");
            ui.add_space(8.0);
            widgets::section_title(ui, &colors, "Theme");
            ui.add_space(4.0);
            for (i, t) in theme::THEMES.iter().enumerate() {
                let preview = match t.palette {
                    None => theme::jade(),
                    Some(p) => theme::derive(&p),
                };
                let selected = picked == i;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
                let fill = if selected { c32(&colors.selection) } else if resp.hovered() { c32(&colors.tab_hover) } else { c32(&colors.control_bg) };
                ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
                ui.painter().text(rect.min + egui::vec2(12.0, 8.0), Align2::LEFT_TOP, t.name, egui::FontId::proportional(13.5), c32(&colors.text));
                ui.painter().text(rect.min + egui::vec2(12.0, 26.0), Align2::LEFT_TOP, t.about, egui::FontId::proportional(10.5), c32(&colors.text_dim));
                for (k, hex) in [&preview.accent, &preview.success_text, &preview.key_color, &preview.danger_text, &preview.ed_function]
                    .iter()
                    .enumerate()
                {
                    let c = egui::pos2(rect.right() - 16.0 - 16.0 * (4 - k) as f32, rect.center().y);
                    ui.painter().circle_filled(c, 6.0, c32(hex));
                }
                if resp.clicked() {
                    picked = i;
                }
            }
            ui.add_space(10.0);
            widgets::section_title(ui, &colors, "Data");
            ui.add_space(4.0);
            egui::Grid::new("settings_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Rows per page");
                ui.add(egui::DragValue::new(&mut d.settings.default_limit).range(10..=10_000).speed(10));
                ui.end_row();
                ui.label("Estimate count above");
                ui.add(egui::DragValue::new(&mut d.settings.estimate_count_threshold).range(1_000..=100_000_000).speed(1000));
                ui.end_row();
                ui.label("Editor font size");
                ui.add(egui::DragValue::new(&mut d.settings.editor_font_size).range(9..=24));
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &colors, "Save", true).clicked() {
                    save = true;
                }
                if widgets::button(ui, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if modal.should_close() {
            cancel = true;
        }
        let d = self.settings_dialog.as_mut().unwrap();
        let changed = picked != d.picked;
        d.picked = picked;
        if changed {
            self.set_theme(ctx, picked);
        }
        if save {
            let d = self.settings_dialog.take().unwrap();
            let mut s = d.settings;
            s.theme = theme::THEMES[d.picked].id.to_string();
            s.last_connection_id = self.env.settings.last_connection_id.clone();
            self.env.settings = s;
            self.env.save_settings();
        } else if cancel {
            let d = self.settings_dialog.take().unwrap();
            self.set_theme(ctx, d.original);
        }
    }

    fn toasts_ui(&mut self, ctx: &egui::Context) {
        self.env.toasts.retain(|t| t.born.elapsed() < Duration::from_secs(if t.ok { 3 } else { 8 }));
        if self.env.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts")).anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).order(egui::Order::Tooltip).show(ctx, |ui| {
            for t in self.env.toasts.iter().rev().take(4) {
                let fill = c32(if t.ok { &self.env.colors.status_ok } else { &self.env.colors.status_err });
                egui::Frame::new()
                    .fill(fill)
                    .corner_radius(CornerRadius::same(8))
                    .stroke(Stroke::new(1.0, egui::Color32::from_white_alpha(30)))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(420.0);
                        ui.label(RichText::new(&t.text).color(egui::Color32::WHITE));
                    });
                ui.add_space(6.0);
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        let modal_open = self.settings_dialog.is_some();
        match &mut self.ws {
            Some(ws) => match ws.ui(ui, &mut self.env, modal_open) {
                Some(workspace::Event::Disconnect) => self.ws = None,
                Some(workspace::Event::Settings) => self.open_settings(),
                Some(workspace::Event::OpenDatabase(db)) => {
                    let cfg = ws.session_config();
                    self.ws = None;
                    self.start_connect(cfg, Some(db));
                }
                None => {}
            },
            None => {
                match self.welcome.ui(ui, &mut self.env, self.theme_index) {
                    Some(welcome::Event::Connect(cfg)) => {
                        if self.connecting.is_none() {
                            self.start_connect(cfg, None);
                        }
                    }
                    Some(welcome::Event::Theme(i)) => {
                        self.set_theme(&ctx, i);
                        self.env.settings.theme = theme::THEMES[i].id.to_string();
                        self.env.save_settings();
                    }
                    None => {}
                }
            }
        }
        self.settings_ui(&ctx);
        self.toasts_ui(&ctx);
        self.run_script(&ctx);
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Comma)) && self.settings_dialog.is_none() {
            self.open_settings();
        }
        let _ = poll_slot::<()>;
    }
}

impl App {
    fn open_settings(&mut self) {
        self.settings_dialog = Some(SettingsDialog { original: self.theme_index, picked: self.theme_index, settings: self.env.settings.clone() });
    }

    /// Dev automation: drives the app through a scripted scenario and saves screenshots.
    fn run_script(&mut self, ctx: &egui::Context) {
        let Some(mut script) = self.script.take() else { return };
        script.step(ctx, self);
        self.script = Some(script);
    }

    pub(crate) fn script_connect(&mut self, name: &str) -> bool {
        match self.welcome.find(name) {
            Some(cfg) => {
                self.start_connect(cfg, None);
                true
            }
            None => false,
        }
    }

    pub(crate) fn script_workspace(&mut self) -> Option<&mut Workspace> {
        self.ws.as_deref_mut()
    }

    pub(crate) fn script_set_theme(&mut self, ctx: &egui::Context, name: &str) {
        if let Some(i) = theme::find(name) {
            self.set_theme(ctx, i);
        }
    }

    pub(crate) fn script_open_settings(&mut self) {
        self.open_settings();
    }

    pub(crate) fn script_env(&mut self) -> &mut Env {
        &mut self.env
    }

    pub(crate) fn script_reload_welcome(&mut self) {
        self.welcome.reload(&self.env);
    }

    pub(crate) fn script_close_settings(&mut self, ctx: &egui::Context) {
        if let Some(d) = self.settings_dialog.take() {
            self.set_theme(ctx, d.picked);
        }
    }

    pub(crate) fn script_with_workspace(&mut self, f: impl FnOnce(&mut Workspace, &mut Env)) {
        if let Some(ws) = self.ws.as_deref_mut() {
            f(ws, &mut self.env);
        }
    }
}
