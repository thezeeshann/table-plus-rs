//! The connection picker and the connection form.

use eframe::egui::{self, Align, Color32, CornerRadius, Layout, RichText, Sense, Stroke, StrokeKind, Ui};
use table_plus_core::model::ConnectionConfig;
use table_plus_core::pg::session;

use crate::app::{err_msg, Env};
use crate::task::{poll_slot, Task};
use crate::theme::{c32, conn_color};
use crate::widgets;

pub const ENVIRONMENTS: [&str; 5] = ["Local", "Development", "Testing", "Staging", "Production"];
const SSL_MODES: [&str; 5] = ["disable", "prefer", "require", "verify-ca", "verify-full"];

pub enum Event {
    Connect(ConnectionConfig),
    Theme(usize),
}

pub struct Welcome {
    connections: Vec<ConnectionConfig>,
    selected: Option<String>,
    form: Option<Form>,
    confirm_delete: Option<String>,
    pub error: Option<String>,
    pub connecting: Option<String>,
}

struct Form {
    cfg: ConnectionConfig,
    editing: bool,
    port_text: String,
    password: String,
    url: String,
    test: Option<Task<String>>,
    status: Option<(bool, String)>,
}

impl Welcome {
    pub fn new(env: &Env) -> Self {
        let mut w = Self { connections: vec![], selected: None, form: None, confirm_delete: None, error: None, connecting: None };
        w.reload(env);
        w
    }

    pub fn reload(&mut self, env: &Env) {
        self.connections = env.storage.list_connections().map(|(c, _)| c).unwrap_or_default();
        if self.selected.is_none() {
            self.selected = env
                .settings
                .last_connection_id
                .clone()
                .filter(|id| self.connections.iter().any(|c| &c.id == id))
                .or_else(|| self.connections.first().map(|c| c.id.clone()));
        }
    }

    pub fn find(&self, name: &str) -> Option<ConnectionConfig> {
        self.connections.iter().find(|c| c.name == name || c.id == name).cloned()
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env, theme: usize) -> Option<Event> {
        let mut event = None;
        let colors = env.colors.clone();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(c32(&colors.content_bg))).show(ui, |ui| {
            ui.add_space(36.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("table-plus-rs").size(26.0).strong());
                ui.label(widgets::dim(ui, "A fast, native PostgreSQL client"));
            });
            ui.add_space(14.0);
            if let Some(i) = theme_picker(ui, &colors, theme) {
                event = Some(Event::Theme(i));
            }
            ui.add_space(18.0);
            let width = 640.0_f32.min(ui.available_width() - 40.0);
            ui.vertical_centered(|ui| {
                ui.allocate_ui(egui::vec2(width, ui.available_height()), |ui| {
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, &colors, "+ New connection", true).clicked() {
                            self.form = Some(Form::new(ConnectionConfig::blank(), false));
                        }
                        let sel = self.selected.clone();
                        if ui.add_enabled(sel.is_some(), egui::Button::new("Edit").min_size(egui::vec2(0.0, 26.0))).clicked() {
                            if let Some(c) = sel.as_ref().and_then(|id| env.storage.get_connection(id).ok()) {
                                self.form = Some(Form::new(c, true));
                            }
                        }
                        if ui.add_enabled(sel.is_some(), egui::Button::new("Delete").min_size(egui::vec2(0.0, 26.0))).clicked() {
                            self.confirm_delete = sel.clone();
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let can = sel.is_some() && self.connecting.is_none();
                            let label = if self.connecting.is_some() { "Connecting…" } else { "Connect" };
                            if widgets::primary_button(ui, &colors, label, can).clicked() {
                                event = sel.as_ref().and_then(|id| env.storage.get_connection(id).ok()).map(Event::Connect);
                            }
                        });
                    });
                    ui.add_space(8.0);
                    if let Some(err) = &self.error {
                        ui.label(RichText::new(err).color(c32(&colors.danger_text)));
                        ui.add_space(6.0);
                    }
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.connections.is_empty() {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| ui.label(widgets::dim(ui, "No connections yet. Create one to get started.")));
                        }
                        for c in self.connections.clone() {
                            if let Some(e) = self.row(ui, env, &c) {
                                event = Some(e);
                            }
                        }
                    });
                });
            });
        });

        self.form_ui(ui.ctx(), env);
        self.delete_ui(ui.ctx(), env);
        event
    }

    fn row(&mut self, ui: &mut Ui, env: &Env, c: &ConnectionConfig) -> Option<Event> {
        let colors = &env.colors;
        let selected = self.selected.as_deref() == Some(&c.id);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 54.0), Sense::click());
        let fill = if selected { c32(&colors.selection) } else if resp.hovered() { c32(&colors.tab_bg) } else { c32(&colors.control_bg) };
        ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
        ui.painter().rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, c32(&colors.control_border)), StrokeKind::Inside);
        let stripe = egui::Rect::from_min_size(rect.min + egui::vec2(0.0, 6.0), egui::vec2(4.0, rect.height() - 12.0));
        ui.painter().rect_filled(stripe, CornerRadius::same(2), conn_color(colors, &c.color));
        let text = c32(&colors.text);
        ui.painter().text(rect.min + egui::vec2(18.0, 10.0), egui::Align2::LEFT_TOP, &c.name, egui::FontId::proportional(14.0), text);
        let detail = format!("{}@{}:{} / {}", c.user, c.host, c.port, c.database);
        ui.painter().text(rect.min + egui::vec2(18.0, 31.0), egui::Align2::LEFT_TOP, detail, egui::FontId::proportional(11.5), c32(&colors.text_dim));
        if !c.environment.is_empty() {
            let col = env_color(colors, &c.environment);
            let galley = ui.painter().layout_no_wrap(c.environment.to_uppercase(), egui::FontId::proportional(10.0), col);
            let r = egui::Rect::from_min_size(
                egui::pos2(rect.right() - galley.size().x - 24.0, rect.center().y - 9.0),
                galley.size() + egui::vec2(12.0, 4.0),
            );
            ui.painter().rect_filled(r, CornerRadius::same(4), col.gamma_multiply(0.2));
            ui.painter().galley(r.center() - galley.size() / 2.0, galley, col);
        }
        if resp.clicked() {
            self.selected = Some(c.id.clone());
        }
        if resp.double_clicked() {
            return env.storage.get_connection(&c.id).ok().map(Event::Connect);
        }
        ui.add_space(4.0);
        None
    }

    fn form_ui(&mut self, ctx: &egui::Context, env: &mut Env) {
        let Some(form) = &mut self.form else { return };
        if let Some(r) = poll_slot(&mut form.test) {
            form.status = Some(match r {
                Ok(v) => (true, format!("Connected: {v}")),
                Err(e) => (false, e.to_string()),
            });
        }
        let colors = env.colors.clone();
        let mut close = false;
        let mut save = false;
        let mut test = false;
        let modal = widgets::modal(ctx, &colors, "conn_form").show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading(if form.editing { "Edit connection" } else { "New connection" });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("From URL");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut form.url).hint_text("postgresql://user:pass@host:5432/db").desired_width(300.0),
                );
                if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || widgets::small_button(ui, "Import").clicked() {
                    if let Some(parsed) = parse_url(&form.url) {
                        form.cfg.host = parsed.host;
                        form.cfg.port = parsed.port;
                        form.port_text = parsed.port.to_string();
                        form.cfg.user = parsed.user;
                        form.cfg.database = parsed.database;
                        form.cfg.ssl_mode = parsed.ssl_mode.unwrap_or(form.cfg.ssl_mode.clone());
                        if let Some(p) = parsed.password {
                            form.password = p;
                        }
                        form.url.clear();
                    } else {
                        form.status = Some((false, "Not a valid postgresql:// URL".into()));
                    }
                }
            });
            ui.add_space(6.0);
            egui::Grid::new("conn_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut form.cfg.name).desired_width(300.0));
                ui.end_row();
                ui.label("Host / Port");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut form.cfg.host).desired_width(210.0));
                    ui.add(egui::TextEdit::singleline(&mut form.port_text).desired_width(80.0));
                });
                ui.end_row();
                ui.label("User");
                ui.add(egui::TextEdit::singleline(&mut form.cfg.user).desired_width(300.0));
                ui.end_row();
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut form.password)
                        .password(true)
                        .hint_text(if form.editing { "leave blank to keep the saved one" } else { "" })
                        .desired_width(300.0),
                );
                ui.end_row();
                ui.label("Database");
                ui.add(egui::TextEdit::singleline(&mut form.cfg.database).desired_width(300.0));
                ui.end_row();
                ui.label("SSL mode");
                egui::ComboBox::from_id_salt("ssl_mode").selected_text(form.cfg.ssl_mode.clone()).show_ui(ui, |ui| {
                    for m in SSL_MODES {
                        ui.selectable_value(&mut form.cfg.ssl_mode, m.to_string(), m);
                    }
                });
                ui.end_row();
                ui.label("Environment");
                egui::ComboBox::from_id_salt("env_tag")
                    .selected_text(if form.cfg.environment.is_empty() { "None" } else { &form.cfg.environment })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut form.cfg.environment, String::new(), "None");
                        for e in ENVIRONMENTS {
                            ui.selectable_value(&mut form.cfg.environment, e.to_string(), e);
                        }
                    });
                ui.end_row();
                ui.label("Color");
                ui.horizontal(|ui| {
                    for tag in 1..=5 {
                        let id = tag.to_string();
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), Sense::click());
                        let col = conn_color(&colors, &id);
                        ui.painter().rect_filled(rect, CornerRadius::same(12), col);
                        if form.cfg.color == id {
                            ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(14), Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);
                        }
                        if resp.clicked() {
                            form.cfg.color = id;
                        }
                    }
                });
                ui.end_row();
                ui.label("Keep alive");
                ui.checkbox(&mut form.cfg.keep_alive, "Ping every 30 s");
                ui.end_row();
            });
            if let Some((ok, msg)) = &form.status {
                ui.add_space(6.0);
                let col = if *ok { c32(&colors.success_text) } else { c32(&colors.danger_text) };
                ui.label(RichText::new(msg).color(col));
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let testing = form.test.is_some();
                if ui.add_enabled(!testing, egui::Button::new(if testing { "Testing…" } else { "Test" }).min_size(egui::vec2(0.0, 26.0))).clicked() {
                    test = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::primary_button(ui, &colors, "Save", !form.cfg.name.trim().is_empty()).clicked() {
                        save = true;
                    }
                    if widgets::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
        });
        if modal.should_close() {
            close = true;
        }

        if test {
            let mut cfg = form.cfg.clone();
            cfg.port = form.port_text.trim().parse().unwrap_or(5432);
            cfg.password = if form.password.is_empty() {
                (!cfg.id.is_empty()).then(|| env.storage.get_connection(&cfg.id).ok().and_then(|c| c.password)).flatten()
            } else {
                Some(form.password.clone())
            };
            form.status = None;
            form.test = Some(env.sp.spawn(async move { session::test(&cfg).await.map_err(err_msg) }));
        }
        if save {
            let mut cfg = form.cfg.clone();
            cfg.port = form.port_text.trim().parse().unwrap_or(5432);
            cfg.password = (!form.password.is_empty()).then(|| form.password.clone());
            match env.storage.save_connection(cfg) {
                Ok(saved) => {
                    self.selected = Some(saved.id);
                    close = true;
                }
                Err(e) => form.status = Some((false, e.payload().one_line())),
            }
        }
        if close {
            self.form = None;
            self.reload(env);
        }
    }

    fn delete_ui(&mut self, ctx: &egui::Context, env: &mut Env) {
        let Some(id) = self.confirm_delete.clone() else { return };
        let name = self.connections.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
        let colors = env.colors.clone();
        let mut done = false;
        let mut confirmed = false;
        widgets::modal(ctx, &colors, "confirm_delete").show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Delete connection?");
            ui.add_space(6.0);
            ui.label(format!("“{name}” and its saved query history will be removed."));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::button(ui, "Cancel").clicked() {
                    done = true;
                }
                if ui.add(egui::Button::new(RichText::new("Delete").color(Color32::WHITE)).fill(c32(&colors.status_err)).min_size(egui::vec2(0.0, 26.0))).clicked() {
                    confirmed = true;
                    done = true;
                }
            });
        });
        if confirmed {
            let _ = env.storage.delete_connection(&id);
            self.selected = None;
            self.reload(env);
        }
        if done {
            self.confirm_delete = None;
        }
    }
}

impl Form {
    fn new(cfg: ConnectionConfig, editing: bool) -> Self {
        let port_text = cfg.port.to_string();
        let mut cfg = cfg;
        cfg.password = None; // never prefill the password field
        Self { cfg, editing, port_text, password: String::new(), url: String::new(), test: None, status: None }
    }
}

/// Row of theme chips shown on the start screen; clicking one applies it immediately.
fn theme_picker(ui: &mut Ui, colors: &crate::theme::Colors, current: usize) -> Option<usize> {
    let chip_w = 124.0;
    let gap = 6.0;
    let n = crate::theme::THEMES.len() as f32;
    let total = n * chip_w + (n - 1.0) * gap;
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing.x = gap;
        for (i, t) in crate::theme::THEMES.iter().enumerate() {
            let preview = crate::theme::colors_for(i);
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(chip_w, 30.0), Sense::click());
            let selected = i == current;
            let fill = if selected { c32(&colors.tab_selected) } else if resp.hovered() { c32(&colors.tab_hover) } else { c32(&colors.control_bg) };
            ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
            ui.painter().rect_stroke(rect, CornerRadius::same(7), Stroke::new(1.0, c32(&colors.control_border)), StrokeKind::Inside);
            for (k, hex) in [&preview.accent, &preview.success_text, &preview.danger_text].iter().enumerate() {
                ui.painter().circle_filled(egui::pos2(rect.left() + 13.0 + k as f32 * 9.0, rect.center().y), 3.5, c32(hex));
            }
            let text_col = if selected { Color32::WHITE } else { c32(&colors.text) };
            ui.painter().text(egui::pos2(rect.left() + 44.0, rect.center().y), egui::Align2::LEFT_CENTER, t.name, egui::FontId::proportional(12.0), text_col);
            if resp.clicked() && !selected {
                picked = Some(i);
            }
            resp.on_hover_text(t.about);
        }
    });
    picked
}

pub fn env_color(colors: &crate::theme::Colors, env: &str) -> Color32 {
    match env {
        "Production" => c32(&colors.danger_text),
        "Staging" => c32(&colors.key_color),
        "Testing" => Color32::from_rgb(0xb5, 0x7a, 0xff),
        "Development" => c32(&colors.success_text),
        _ => c32(&colors.text_dim),
    }
}

pub struct ParsedUrl {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub database: String,
    pub ssl_mode: Option<String>,
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `postgresql://user:pass@host:5432/db?sslmode=require`
pub fn parse_url(url: &str) -> Option<ParsedUrl> {
    let rest = url.trim().strip_prefix("postgresql://").or_else(|| url.trim().strip_prefix("postgres://"))?;
    let (main, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (auth, hostpart) = match main.rsplit_once('@') {
        Some((a, h)) => (Some(a), h),
        None => (None, main),
    };
    let (hostport, db) = hostpart.split_once('/').unwrap_or((hostpart, ""));
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (hostport, 5432),
    };
    if host.is_empty() {
        return None;
    }
    let (user, password) = match auth {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (percent_decode(u), Some(percent_decode(p))),
            None => (percent_decode(a), None),
        },
        None => (String::new(), None),
    };
    let ssl_mode = query.split('&').find_map(|kv| kv.strip_prefix("sslmode=")).map(String::from);
    Some(ParsedUrl { host: host.into(), port, user, password, database: percent_decode(db), ssl_mode })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls() {
        let p = parse_url("postgresql://postgres.abc:p%40ss@aws-0.pooler.supabase.com:6543/postgres?sslmode=require").unwrap();
        assert_eq!((p.host.as_str(), p.port, p.user.as_str(), p.database.as_str()), ("aws-0.pooler.supabase.com", 6543, "postgres.abc", "postgres"));
        assert_eq!(p.password.as_deref(), Some("p@ss"));
        assert_eq!(p.ssl_mode.as_deref(), Some("require"));
        let q = parse_url("postgres://localhost/db").unwrap();
        assert_eq!((q.host.as_str(), q.port), ("localhost", 5432));
        assert!(parse_url("mysql://x").is_none());
        assert!(parse_url("postgresql://@/db").is_none());
    }
}
