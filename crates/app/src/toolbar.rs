//! Top toolbar. The centre status pill shows where you are and doubles as the loading
//! indicator: while the active tab is busy it fills with the connection's own color.

use eframe::egui::{self, epaint::Vertex, Align, Align2, Color32, CornerRadius, Layout, Mesh, Sense, Stroke, StrokeKind, Ui};

use crate::theme::{c32, conn_color, Colors};

pub struct Data<'a> {
    pub colors: &'a Colors,
    pub color_tag: &'a str,
    pub environment: &'a str,
    pub server: &'a str,
    pub tls: Option<&'a str>,
    pub database: &'a str,
    pub user: &'a str,
    /// "schema.table" or "SQL" for the active tab.
    pub where_: &'a str,
    pub is_query: bool,
    pub busy: bool,
    pub alive: bool,
    pub reconnecting: bool,
    pub sidebar_open: bool,
    pub mode: crate::sidebar::Mode,
}

#[derive(Default)]
pub struct Out {
    pub toggle_sidebar: bool,
    pub reload: bool,
    pub new_query: bool,
    pub processes: bool,
    pub settings: bool,
    pub disconnect: bool,
    pub open_database: bool,
    pub reconnect: bool,
    pub mode: Option<crate::sidebar::Mode>,
    pub add_row: bool,
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

#[derive(Clone, Copy)]
enum Icon {
    Sidebar,
    Database,
    Star,
    Clock,
    Lock,
    Terminal,
    Reload,
    Plus,
    Pulse,
    Gear,
    Power,
}

fn arc(c: egui::Pos2, r: f32, from_deg: f32, to_deg: f32) -> Vec<egui::Pos2> {
    let n = 20;
    (0..=n)
        .map(|i| {
            let a = (from_deg + (to_deg - from_deg) * i as f32 / n as f32).to_radians();
            c + egui::vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

fn paint_icon(p: &egui::Painter, c: egui::Pos2, icon: Icon, color: Color32) {
    let st = Stroke::new(1.4, color);
    let v = egui::vec2;
    match icon {
        Icon::Sidebar => {
            let r = egui::Rect::from_center_size(c, v(14.0, 12.0));
            p.rect_stroke(r, CornerRadius::same(2), st, StrokeKind::Inside);
            let x = r.left() + 5.0;
            p.line_segment([egui::pos2(x, r.top()), egui::pos2(x, r.bottom())], st);
        }
        Icon::Database => {
            let top = c + v(0.0, -4.5);
            let ell = |y: f32, from: f32, to: f32| arc(c + v(0.0, y), 1.0, from, to);
            let _ = ell;
            let ellipse = |cy: f32, from: f32, to: f32| -> Vec<egui::Pos2> {
                (0..=24).map(|i| {
                    let a = (from + (to - from) * i as f32 / 24.0).to_radians();
                    egui::pos2(c.x + a.cos() * 5.5, cy + a.sin() * 2.2)
                }).collect()
            };
            p.add(egui::Shape::line(ellipse(top.y, 0.0, 360.0), st));
            p.add(egui::Shape::line(ellipse(c.y, 0.0, 180.0), st));
            p.add(egui::Shape::line(ellipse(c.y + 4.5, 0.0, 180.0), st));
            p.line_segment([egui::pos2(c.x - 5.5, top.y), egui::pos2(c.x - 5.5, c.y + 4.5)], st);
            p.line_segment([egui::pos2(c.x + 5.5, top.y), egui::pos2(c.x + 5.5, c.y + 4.5)], st);
        }
        Icon::Star => {
            let pts: Vec<egui::Pos2> = (0..10)
                .map(|i| {
                    let r = if i % 2 == 0 { 6.5 } else { 2.8 };
                    let a = (-90.0 + i as f32 * 36.0).to_radians();
                    c + v(a.cos() * r, a.sin() * r + 0.5)
                })
                .collect();
            p.add(egui::Shape::closed_line(pts, st));
        }
        Icon::Clock => {
            p.add(egui::Shape::line(arc(c, 6.0, 40.0, 340.0), st));
            p.add(egui::Shape::line(vec![c + v(-6.0, -3.5), c + v(-6.0, -0.5), c + v(-3.2, -1.2)], st));
            p.line_segment([c, c + v(0.0, -3.0)], st);
            p.line_segment([c, c + v(2.5, 1.5)], st);
        }
        Icon::Lock => {
            let body = egui::Rect::from_center_size(c + v(0.0, 2.0), v(11.0, 8.0));
            p.rect_stroke(body, CornerRadius::same(2), st, StrokeKind::Inside);
            p.add(egui::Shape::line(arc(c + v(0.0, -2.0), 3.5, 180.0, 360.0), st));
            p.line_segment([c + v(-3.5, -2.0), c + v(-3.5, -1.5)], st);
            p.line_segment([c + v(3.5, -2.0), c + v(3.5, -1.5)], st);
        }
        Icon::Terminal => {
            p.add(egui::Shape::line(vec![c + v(-6.0, -4.0), c + v(-2.0, 0.0), c + v(-6.0, 4.0)], st));
            p.line_segment([c + v(0.0, 4.5), c + v(6.0, 4.5)], st);
        }
        Icon::Reload => {
            p.add(egui::Shape::line(arc(c, 5.5, -60.0, 230.0), st));
            let tip = c + v(5.5 * (-60f32).to_radians().cos(), 5.5 * (-60f32).to_radians().sin());
            p.add(egui::Shape::line(vec![tip + v(-4.0, -0.5), tip, tip + v(0.5, 4.0)], st));
        }
        Icon::Plus => {
            p.line_segment([c - v(6.0, 0.0), c + v(6.0, 0.0)], st);
            p.line_segment([c - v(0.0, 6.0), c + v(0.0, 6.0)], st);
        }
        Icon::Pulse => {
            p.add(egui::Shape::line(vec![c + v(-7.0, 0.0), c + v(-3.5, 0.0), c + v(-1.5, -6.0), c + v(2.0, 6.0), c + v(4.0, 0.0), c + v(7.0, 0.0)], st));
        }
        Icon::Gear => {
            p.circle_stroke(c, 2.3, st);
            for i in 0..8 {
                let a = (i as f32 * 45.0).to_radians();
                let (d, e) = (v(a.cos(), a.sin()), 0.0);
                let _ = e;
                p.line_segment([c + d * 5.4, c + d * 7.2], Stroke::new(2.0, color));
            }
            p.circle_stroke(c, 5.2, st);
        }
        Icon::Power => {
            p.add(egui::Shape::line(arc(c + v(0.0, 0.5), 5.8, -50.0, 230.0), st));
            p.line_segment([c + v(0.0, -7.0), c + v(0.0, -1.0)], st);
        }
    }
}

pub fn paint_database(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    paint_icon(p, c, Icon::Database, color);
}

fn icon_btn(ui: &mut Ui, colors: &Colors, icon: Icon, tip: &str, active: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(30.0, 26.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, CornerRadius::same(13), c32(&colors.tab_selected).gamma_multiply(0.8));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(13), c32(&colors.tab_hover));
    }
    paint_icon(ui.painter(), rect.center(), icon, c32(&colors.text));
    resp.on_hover_text(tip).clicked()
}

/// A cluster of icon buttons on one rounded background, as TablePlus groups its toolbar.
fn group(ui: &mut Ui, colors: &Colors, add: impl FnOnce(&mut Ui)) {
    let bg = ui.painter().add(egui::Shape::Noop);
    let rect = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(3.0);
            add(ui);
            ui.add_space(3.0);
        })
        .response
        .rect;
    let cy = ui.max_rect().center().y;
    let rect = egui::Rect::from_min_max(egui::pos2(rect.left(), cy - 17.0), egui::pos2(rect.right(), cy + 17.0));
    ui.painter().set(bg, egui::epaint::RectShape::filled(rect, CornerRadius::same(17), c32(&colors.toolbar_group)));
}

pub fn show(ui: &mut Ui, d: &Data) -> Out {
    let mut out = Out::default();
    let colors = d.colors;
    ui.horizontal_centered(|ui| {
        use crate::sidebar::Mode;
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(12.0);
        group(ui, colors, |ui| {
            if icon_btn(ui, colors, Icon::Sidebar, "Toggle sidebar (Cmd+\\)", false) {
                out.toggle_sidebar = true;
            }
        });
        group(ui, colors, |ui| {
            for (mode, icon, tip) in [(Mode::Items, Icon::Database, "Items"), (Mode::Queries, Icon::Star, "Saved queries"), (Mode::History, Icon::Clock, "History")] {
                if icon_btn(ui, colors, icon, tip, d.sidebar_open && d.mode == mode) {
                    out.mode = Some(mode);
                }
            }
        });
        group(ui, colors, |ui| {
            if icon_btn(ui, colors, Icon::Lock, "Open database (Cmd+K)", false) {
                out.open_database = true;
            }
            if icon_btn(ui, colors, Icon::Terminal, "New SQL query (Cmd+T)", false) {
                out.new_query = true;
            }
        });
        ui.add_space(2.0);

        // centre the pill in the window: equal room on both sides, sized by the busier side
        let full = ui.max_rect();
        let right_w = 5.0 * 30.0 + 6.0 + 20.0;
        let side = (ui.cursor().left() - full.left()).max(right_w) + 8.0;
        let pill_w = (full.width() - 2.0 * side).clamp(220.0, 820.0);
        ui.add_space((full.center().x - pill_w / 2.0 - ui.cursor().left()).max(0.0));
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(pill_w, 26.0), Sense::click());
        let tag = conn_color(colors, d.color_tag);
        let t = ui.ctx().animate_bool_with_time(egui::Id::new("pill_busy"), d.busy && d.alive, 0.16);

        let base = c32(&colors.control_bg);
        let (fill, border, text_col) = if d.reconnecting {
            (c32(&colors.status_warn), c32(&colors.status_warn), Color32::WHITE)
        } else if !d.alive {
            (c32(&colors.status_err), c32(&colors.status_err), Color32::WHITE)
        } else {
            (lerp(base, tag, t), lerp(c32(&colors.control_border), tag, t), lerp(c32(&colors.text), Color32::WHITE, t))
        };
        let radius = CornerRadius::same(7);
        ui.painter().rect_filled(rect, radius, fill);
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, border), StrokeKind::Inside);
        // 4px stripe in the connection color (hidden once the whole pill carries it)
        if t < 0.99 && d.alive && !d.reconnecting {
            let stripe = egui::Rect::from_min_size(rect.min, egui::vec2(5.0, rect.height()));
            ui.painter().rect_filled(stripe, CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 }, tag);
        }
        // sweeping highlight while loading
        if t > 0.01 {
            let time = ui.input(|i| i.time) as f32;
            let w = 90.0;
            let x = rect.left() - w + ((time * 0.9) % 1.0) * (rect.width() + w);
            let a = (t * 70.0) as u8;
            let (c0, c1) = (Color32::from_white_alpha(0), Color32::from_white_alpha(a));
            let mut mesh = Mesh::default();
            let (y0, y1) = (rect.top(), rect.bottom());
            for (px, c) in [(x, c0), (x + w / 2.0, c1), (x + w, c0)] {
                mesh.vertices.push(Vertex { pos: egui::pos2(px, y0), uv: egui::epaint::WHITE_UV, color: c });
                mesh.vertices.push(Vertex { pos: egui::pos2(px, y1), uv: egui::epaint::WHITE_UV, color: c });
            }
            mesh.indices.extend([0, 1, 2, 1, 2, 3, 2, 3, 4, 3, 4, 5]);
            ui.painter().with_clip_rect(rect.shrink(1.0)).add(egui::Shape::mesh(mesh));
            ui.ctx().request_repaint();
        }

        let text = if d.reconnecting {
            "Reconnecting…".to_string()
        } else if !d.alive {
            "Disconnected: click to reconnect".to_string()
        } else {
            let env = if d.environment.is_empty() { "Local" } else { d.environment };
            if d.busy {
                format!("{env}  |  {}…", if d.is_query { "Running query".to_string() } else { format!("Loading {}", d.where_) })
            } else {
                let tls = d.tls.map(|t| format!(" : {t}")).unwrap_or_default();
                format!("{env}  |  PostgreSQL {}{tls} : {} : {} : {}", d.server, d.database, d.user, d.where_)
            }
        };
        let font = crate::theme::mono(12.0); // TablePlus sets the pill in SF Mono
        let shown = {
            // keep it on one line inside the pill
            let max = ((pill_w - 28.0) / 7.3) as usize;
            if text.chars().count() > max { text.chars().take(max.saturating_sub(1)).collect::<String>() + "…" } else { text }
        };
        let center = rect.center() + egui::vec2(3.0, 0.0);
        if t > 0.5 {
            ui.painter().text(center + egui::vec2(0.0, 1.0), Align2::CENTER_CENTER, &shown, font.clone(), Color32::from_black_alpha(90));
        }
        ui.painter().text(center, Align2::CENTER_CENTER, &shown, font, text_col);
        if resp.clicked() && !d.alive && !d.reconnecting {
            out.reconnect = true;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            group(ui, colors, |ui| {
                if icon_btn(ui, colors, Icon::Power, "Disconnect", false) {
                    out.disconnect = true;
                }
                if icon_btn(ui, colors, Icon::Gear, "Settings (Cmd+,)", false) {
                    out.settings = true;
                }
                if icon_btn(ui, colors, Icon::Pulse, "Process list (Cmd+.)", false) {
                    out.processes = true;
                }
                if icon_btn(ui, colors, Icon::Plus, "Add row (Cmd+I)", false) {
                    out.add_row = true;
                }
                if icon_btn(ui, colors, Icon::Reload, "Reload (Cmd+R)", false) {
                    out.reload = true;
                }
            });
        });
    });
    out
}
