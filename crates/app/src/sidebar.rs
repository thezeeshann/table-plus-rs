//! Left sidebar: Items (tables, views, functions…), saved Queries and History.

use eframe::egui::{self, CornerRadius, RichText, Sense, Ui};
use table_plus_core::model::{FavoriteQuery, HistoryEntry, SchemaListing};

use crate::theme::{c32, Colors};
use crate::widgets::{self, IconKind};
use crate::workspace::{Action, Destructive};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Items,
    Queries,
    History,
}

pub struct State {
    pub mode: Mode,
    pub search: String,
}

impl Default for State {
    fn default() -> Self {
        Self { mode: Mode::Items, search: String::new() }
    }
}

pub struct Data<'a> {
    pub listing: Option<&'a SchemaListing>,
    pub loading: bool,
    pub schemas: &'a [String],
    pub schema: &'a str,
    pub favorites: &'a [FavoriteQuery],
    pub history: &'a [HistoryEntry],
    /// (schema, name) of the table open in the active tab.
    pub active: Option<(&'a str, &'a str)>,
    pub colors: &'a Colors,
}

fn row(ui: &mut Ui, colors: &Colors, icon: Option<IconKind>, label: &str, right: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.selection));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.control_bg));
    }
    let fg = if selected { egui::Color32::WHITE } else { c32(&colors.text) };
    let mut x = rect.left() + if icon.is_some() { 20.0 } else { 8.0 };
    if let Some(kind) = icon {
        widgets::paint_tab_icon(ui.painter(), egui::pos2(x + 6.5, rect.center().y), kind, fg.gamma_multiply(0.8));
        x += 20.0;
    }
    let right_pad = if right.is_empty() { 6.0 } else { 54.0 };
    let clip = egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(rect.right() - right_pad, rect.bottom()));
    let font = if icon.is_some() { crate::theme::mono(12.0) } else { egui::FontId::proportional(13.0) };
    ui.painter().with_clip_rect(clip).text(egui::pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, label, font, fg);
    if !right.is_empty() {
        let dim = if selected { fg.gamma_multiply(0.75) } else { c32(&colors.text_dim) };
        ui.painter().text(egui::pos2(rect.right() - 6.0, rect.center().y), egui::Align2::RIGHT_CENTER, right, egui::FontId::proportional(11.0), dim);
    }
    resp
}

/// Collapsible group header: chevron, title in SF Mono (as TablePlus), dim count on the right. Returns whether it is open.
fn group_header(ui: &mut Ui, colors: &Colors, id: egui::Id, title: &str, count: usize, default_open: bool) -> bool {
    let open = ui.memory_mut(|m| *m.data.get_temp_mut_or_insert_with(id, || default_open));
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 18.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.control_bg));
    }
    let dim = c32(&colors.text_dim);
    let c = egui::pos2(rect.left() + 12.0, rect.center().y);
    let st = egui::Stroke::new(1.3, c32(&colors.text_dim));
    let v = egui::vec2;
    if open {
        ui.painter().add(egui::Shape::line(vec![c + v(-3.5, -1.5), c + v(0.0, 2.0), c + v(3.5, -1.5)], st));
    } else {
        ui.painter().add(egui::Shape::line(vec![c + v(-1.5, -3.5), c + v(2.0, 0.0), c + v(-1.5, 3.5)], st));
    }
    ui.painter().text(egui::pos2(rect.left() + 24.0, rect.center().y), egui::Align2::LEFT_CENTER, title, crate::theme::mono(12.0), c32(&colors.text));
    ui.painter().text(egui::pos2(rect.right() - 8.0, rect.center().y), egui::Align2::RIGHT_CENTER, count.to_string(), egui::FontId::proportional(11.0), dim);
    if resp.clicked() {
        ui.memory_mut(|m| m.data.insert_temp(id, !open));
        return !open;
    }
    open
}

fn est(n: Option<i64>) -> String {
    match n {
        Some(n) if n >= 0 => {
            if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1e6) } else if n >= 1000 { format!("{:.1}k", n as f64 / 1e3) } else { n.to_string() }
        }
        _ => String::new(),
    }
}

pub fn show(ui: &mut Ui, st: &mut State, d: &Data, actions: &mut Vec<Action>) {
    let colors = d.colors;
    egui::Panel::bottom("sidebar_bottom")
        .exact_size(44.0)
        .frame(egui::Frame::new().fill(c32(&colors.sidebar_bg)).inner_margin(egui::Margin::symmetric(8, 7)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.tab_hover));
                }
                let c = rect.center();
                let stroke = egui::Stroke::new(1.5, c32(&colors.text));
                ui.painter().line_segment([c - egui::vec2(5.0, 0.0), c + egui::vec2(5.0, 0.0)], stroke);
                ui.painter().line_segment([c - egui::vec2(0.0, 5.0), c + egui::vec2(0.0, 5.0)], stroke);
                if resp.on_hover_text("New table").clicked() {
                    actions.push(Action::CreateTable);
                }
                let mut schema = d.schema.to_string();
                egui::ComboBox::from_id_salt("schema_combo").width(ui.available_width() - 14.0).selected_text(&schema).show_ui(ui, |ui| {
                    for s in d.schemas {
                        ui.selectable_value(&mut schema, s.clone(), s);
                    }
                });
                if schema != d.schema {
                    actions.push(Action::SwitchSchema(schema));
                }
            });
        });

    // mode switch: plain words, the active one filled
    egui::Panel::top("sidebar_modes")
        .exact_size(36.0)
        .frame(egui::Frame::new().fill(c32(&colors.sidebar_bg)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let font = egui::FontId::proportional(13.0);
                let items = [(Mode::Items, "Items"), (Mode::Queries, "Queries"), (Mode::History, "History")];
                let widths: Vec<f32> = items.iter().map(|(_, l)| ui.painter().layout_no_wrap((*l).into(), font.clone(), egui::Color32::WHITE).size().x + 24.0).collect();
                let total: f32 = widths.iter().sum::<f32>() + 4.0 * 2.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(4.0));
                ui.spacing_mut().item_spacing.x = 4.0;
                for ((mode, label), w) in items.iter().zip(widths) {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 22.0), Sense::click());
                    let on = st.mode == *mode;
                    if on {
                        ui.painter().rect_filled(rect, CornerRadius::same(5), c32(&colors.mode_selected));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(4), c32(&colors.tab_hover));
                    }
                    let col = if on { egui::Color32::WHITE } else { c32(&colors.tab_text) };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), col);
                    if resp.clicked() {
                        st.mode = *mode;
                    }
                }
            });
        });

    ui.add_space(8.0);
    if st.mode == Mode::Items {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let w = ui.available_width() - 8.0;
            let resp = ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search for item...").desired_width(w).margin(egui::Margin::symmetric(26, 4)));
            // magnifier inside the field
            let c = egui::pos2(resp.rect.left() + 13.0, resp.rect.center().y - 1.0);
            let s = egui::Stroke::new(1.2, c32(&colors.text_dim));
            ui.painter().circle_stroke(c, 4.0, s);
            ui.painter().line_segment([c + egui::vec2(3.0, 3.0), c + egui::vec2(6.0, 6.0)], s);
        });
        ui.add_space(6.0);
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(2.0);
        ui.spacing_mut().item_spacing.y = 1.0;
        match st.mode {
            Mode::Items => items(ui, st, d, actions),
            Mode::Queries => {
                if d.favorites.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| ui.label(widgets::dim(ui, "No saved queries.\nUse ★ Save query in a SQL tab.")));
                }
                for f in d.favorites {
                    let r = row(ui, colors, None, &f.name, "", false);
                    if r.clicked() {
                        actions.push(Action::OpenQuery { title: f.name.clone(), sql: f.sql.clone() });
                    }
                    r.context_menu(|ui| {
                        if ui.button("Open").clicked() {
                            actions.push(Action::OpenQuery { title: f.name.clone(), sql: f.sql.clone() });
                            ui.close();
                        }
                        if ui.button("Delete").clicked() {
                            actions.push(Action::DeleteFavorite(f.id.clone()));
                            ui.close();
                        }
                    });
                }
            }
            Mode::History => {
                if !d.history.is_empty() && widgets::small_button(ui, "Clear history").clicked() {
                    actions.push(Action::ClearHistory);
                }
                for h in d.history.iter().take(200) {
                    let one: String = h.sql.split_whitespace().collect::<Vec<_>>().join(" ");
                    let r = row(ui, colors, None, &one, if h.ok { "" } else { "✗" }, false);
                    let r = r.on_hover_text(format!("{}\n{} · {:.0} ms", h.sql, h.at, h.duration_ms));
                    if r.clicked() {
                        actions.push(Action::OpenQuery { title: "History".into(), sql: h.sql.clone() });
                    }
                }
            }
        }
    });
}

fn items(ui: &mut Ui, st: &State, d: &Data, actions: &mut Vec<Action>) {
    let colors = d.colors;
    let Some(l) = d.listing else {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            if d.loading {
                ui.spinner();
            } else {
                ui.label(widgets::dim(ui, "No items"));
            }
        });
        return;
    };
    let q = st.search.trim().to_lowercase();
    let hit = |n: &str| q.is_empty() || n.to_lowercase().contains(&q);

    let sections: [(&str, Vec<&str>); 4] = [
        ("Tables", vec!["table", "partitioned"]),
        ("Views", vec!["view"]),
        ("Materialized Views", vec!["matview"]),
        ("Foreign tables", vec!["foreign"]),
    ];
    for (title, kinds) in sections {
        let list: Vec<_> = l.items.iter().filter(|i| kinds.contains(&i.kind.as_str()) && hit(&i.name)).collect();
        if list.is_empty() {
            continue;
        }
        let open = group_header(ui, colors, egui::Id::new(("sec", title)), title, list.len(), title == "Tables" || title == "Views" || !q.is_empty());
        if open {
            {
                for it in list {
                    let selected = d.active == Some((it.schema.as_str(), it.name.as_str()));
                    let r = row(ui, colors, Some(IconKind::of_item(&it.kind)), &it.name, &est(it.estimated_rows), selected);
                    let open = |pin: bool| Action::OpenTable { schema: it.schema.clone(), name: it.name.clone(), kind: it.kind.clone(), filter: None, pin };
                    if r.double_clicked() {
                        actions.push(open(true));
                    } else if r.clicked() {
                        actions.push(open(false));
                    }
                    r.context_menu(|ui| {
                        if ui.button("Open").clicked() {
                            actions.push(open(false));
                            ui.close();
                        }
                        if ui.button("Open in new tab").clicked() {
                            actions.push(open(true));
                            ui.close();
                        }
                        if ui.button("Copy name").clicked() {
                            ui.ctx().copy_text(it.name.clone());
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("Rename…").clicked() {
                            actions.push(Action::Rename { schema: it.schema.clone(), name: it.name.clone(), kind: it.kind.clone() });
                            ui.close();
                        }
                        if it.kind == "matview" && ui.button("Refresh materialized view").clicked() {
                            actions.push(Action::Destructive(Destructive::refresh_matview(&it.schema, &it.name)));
                            ui.close();
                        }
                        if matches!(it.kind.as_str(), "table" | "partitioned") && ui.button("Truncate…").clicked() {
                            actions.push(Action::Destructive(Destructive::truncate(&it.schema, &it.name)));
                            ui.close();
                        }
                        if ui.button(RichText::new("Drop…").color(c32(&colors.danger_text))).clicked() {
                            actions.push(Action::Destructive(Destructive::drop(&it.schema, &it.name, &it.kind)));
                            ui.close();
                        }
                    });
                }
            }
        }
    }
    let fns: Vec<_> = l.functions.iter().filter(|f| hit(&f.name)).collect();
    if !fns.is_empty() {
        if group_header(ui, colors, egui::Id::new("sec_fn"), "Functions", fns.len(), !q.is_empty()) {
            for f in fns {
                let r = row(ui, colors, Some(IconKind::Function), &format!("{}({})", f.name, f.args), "", false);
                if r.clicked() {
                    actions.push(Action::OpenDefinition { schema: f.schema.clone(), name: f.name.clone(), kind: "function".into() });
                }
            }
        }
    }
    let seqs: Vec<_> = l.sequences.iter().filter(|s| hit(&s.name)).collect();
    if !seqs.is_empty() {
        if group_header(ui, colors, egui::Id::new("sec_seq"), "Sequences", seqs.len(), !q.is_empty()) {
            for s in seqs {
                let r = row(ui, colors, Some(IconKind::Sequence), &s.name, s.last_value.as_deref().unwrap_or(""), false);
                if r.clicked() {
                    actions.push(Action::OpenDefinition { schema: s.schema.clone(), name: s.name.clone(), kind: "sequence".into() });
                }
            }
        }
    }
}
