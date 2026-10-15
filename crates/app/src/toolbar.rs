//! Top toolbar. The centre status pill shows where you are and doubles as the loading
//! indicator: while the active tab is busy it fills with the connection's own color.

use eframe::egui::{self, epaint::Vertex, Align, Align2, Color32, CornerRadius, FontId, Layout, Mesh, Sense, Stroke, StrokeKind, Ui};

use crate::theme::{c32, conn_color, Colors};
use crate::widgets;

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
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

#[derive(Clone, Copy)]
enum Icon {
    SidebarOpen,
    SidebarClosed,
    Plus,
    Power,
    Text(&'static str),
}

fn paint_icon(p: &egui::Painter, c: egui::Pos2, icon: Icon, color: Color32) {
    let st = Stroke::new(1.6, color);
    match icon {
        Icon::SidebarOpen | Icon::SidebarClosed => {
            let r = egui::Rect::from_center_size(c, egui::vec2(15.0, 12.0));
            p.rect_stroke(r, CornerRadius::same(2), st, StrokeKind::Inside);
            let x = r.left() + 5.5;
            p.line_segment([egui::pos2(x, r.top()), egui::pos2(x, r.bottom())], st);
            if matches!(icon, Icon::SidebarOpen) {
                p.rect_filled(egui::Rect::from_min_max(r.min + egui::vec2(1.0, 1.0), egui::pos2(x - 0.5, r.bottom() - 1.0)), CornerRadius::ZERO, color.gamma_multiply(0.7));
            }
        }
        Icon::Plus => {
            p.line_segment([c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)], st);
            p.line_segment([c - egui::vec2(0.0, 6.0), c + egui::vec2(0.0, 6.0)], st);
        }
        Icon::Power => {
            let pts: Vec<egui::Pos2> = (0..=18)
                .map(|i| {
                    let a = (-60.0 + i as f32 * (300.0 / 18.0)).to_radians() - std::f32::consts::FRAC_PI_2 + 0.0;
                    c + egui::vec2(a.cos(), a.sin()) * 6.0
                })
                .collect();
            p.add(egui::Shape::line(pts, st));
            p.line_segment([c - egui::vec2(0.0, 7.0), c], st);
        }
        Icon::Text(t) => {
            p.text(c, Align2::CENTER_CENTER, t, FontId::proportional(14.0), color);
        }
    }
}

fn icon_btn(ui: &mut Ui, colors: &Colors, icon: Icon, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(28.0, 26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.tab_hover));
    }
    paint_icon(ui.painter(), rect.center(), icon, c32(&colors.tab_text));
    resp.on_hover_text(tip).clicked()
}

pub fn show(ui: &mut Ui, d: &Data) -> Out {
    let mut out = Out::default();
    let colors = d.colors;
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        if icon_btn(ui, colors, if d.sidebar_open { Icon::SidebarOpen } else { Icon::SidebarClosed }, "Toggle sidebar (Cmd+\\)") {
            out.toggle_sidebar = true;
        }
        if widgets::small_button(ui, &format!("{}  …", d.database)).on_hover_text("Open database (Cmd+K)").clicked() {
            out.open_database = true;
        }

        let right_w = 4.0 * 30.0 + 40.0;
        let pill_w = (ui.available_width() - right_w).clamp(220.0, 720.0);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(pill_w, 28.0), Sense::click());
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
        let font = FontId::proportional(12.0);
        let shown = {
            // keep it on one line inside the pill
            let max = ((pill_w - 28.0) / 6.3) as usize;
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
            ui.add_space(8.0);
            if icon_btn(ui, colors, Icon::Power, "Disconnect") {
                out.disconnect = true;
            }
            if icon_btn(ui, colors, Icon::Text("⚙"), "Settings (Cmd+,)") {
                out.settings = true;
            }
            if icon_btn(ui, colors, Icon::Text("☰"), "Process list (Cmd+.)") {
                out.processes = true;
            }
            if icon_btn(ui, colors, Icon::Plus, "New SQL tab (Cmd+T)") {
                out.new_query = true;
            }
            if icon_btn(ui, colors, Icon::Text("⟳"), "Reload (Cmd+R)") {
                out.reload = true;
            }
        });
    });
    out
}
