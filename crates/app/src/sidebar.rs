//! Left sidebar: Items (tables, views, functions…), saved Queries and History.

use eframe::egui::{self, Align, CornerRadius, Layout, RichText, Sense, Ui};
use table_plus_core::model::{FavoriteQuery, HistoryEntry, SchemaListing};

use crate::theme::{c32, Colors};
use crate::widgets;
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

fn row(ui: &mut Ui, colors: &Colors, label: &str, right: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, CornerRadius::same(4), c32(&colors.selection));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), c32(&colors.tab_bg));
    }
    let text_rect = rect.shrink2(egui::vec2(8.0, 0.0));
    let painter = ui.painter().with_clip_rect(text_rect.with_max_x(text_rect.right() - if right.is_empty() { 0.0 } else { 54.0 }));
    painter.text(text_rect.left_center(), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), c32(&colors.text));
    if !right.is_empty() {
        ui.painter().text(text_rect.right_center(), egui::Align2::RIGHT_CENTER, right, egui::FontId::proportional(10.5), c32(&colors.text_dim));
    }
    resp
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
    egui::Panel::bottom("sidebar_bottom").exact_size(40.0).frame(egui::Frame::new().fill(c32(&colors.sidebar_bg)).inner_margin(egui::Margin::symmetric(10, 7))).show(ui, |ui| {
        ui.horizontal(|ui| {
            let mut schema = d.schema.to_string();
            egui::ComboBox::from_id_salt("schema_combo").width(150.0).selected_text(&schema).show_ui(ui, |ui| {
                for s in d.schemas {
                    ui.selectable_value(&mut schema, s.clone(), s);
                }
            });
            if schema != d.schema {
                actions.push(Action::SwitchSchema(schema));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::small_button(ui, "+ Table").clicked() {
                    actions.push(Action::CreateTable);
                }
                if widgets::small_button(ui, "⟳").on_hover_text("Refresh").clicked() {
                    actions.push(Action::RefreshListing);
                }
            });
        });
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        widgets::segmented(ui, colors, &mut st.mode, &[(Mode::Items, "Items"), (Mode::Queries, "Queries"), (Mode::History, "History")]);
    });
    ui.add_space(6.0);
    if st.mode == Mode::Items {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Search for item…").desired_width(ui.available_width() - 10.0));
        });
        ui.add_space(4.0);
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
                    let r = row(ui, colors, &f.name, "", false);
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
                    let r = row(ui, colors, &one, if h.ok { "" } else { "✗" }, false);
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
        ("Materialized views", vec!["matview"]),
        ("Foreign tables", vec!["foreign"]),
    ];
    for (title, kinds) in sections {
        let list: Vec<_> = l.items.iter().filter(|i| kinds.contains(&i.kind.as_str()) && hit(&i.name)).collect();
        if list.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(RichText::new(format!("{title}  {}", list.len())).size(11.5).strong().color(c32(&colors.text_dim)))
            .default_open(title == "Tables" || !q.is_empty())
            .id_salt(("sec", title))
            .show(ui, |ui| {
                for it in list {
                    let selected = d.active == Some((it.schema.as_str(), it.name.as_str()));
                    let r = row(ui, colors, &it.name, &est(it.estimated_rows), selected);
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
            });
    }
    let fns: Vec<_> = l.functions.iter().filter(|f| hit(&f.name)).collect();
    if !fns.is_empty() {
        egui::CollapsingHeader::new(RichText::new(format!("Functions  {}", fns.len())).size(11.5).strong().color(c32(&colors.text_dim))).id_salt("sec_fn").default_open(!q.is_empty()).show(ui, |ui| {
            for f in fns {
                let r = row(ui, colors, &format!("{}({})", f.name, f.args), "", false);
                if r.clicked() {
                    actions.push(Action::OpenDefinition { schema: f.schema.clone(), name: f.name.clone(), kind: "function".into() });
                }
            }
        });
    }
    let seqs: Vec<_> = l.sequences.iter().filter(|s| hit(&s.name)).collect();
    if !seqs.is_empty() {
        egui::CollapsingHeader::new(RichText::new(format!("Sequences  {}", seqs.len())).size(11.5).strong().color(c32(&colors.text_dim))).id_salt("sec_seq").default_open(!q.is_empty()).show(ui, |ui| {
            for s in seqs {
                let r = row(ui, colors, &s.name, s.last_value.as_deref().unwrap_or(""), false);
                if r.clicked() {
                    actions.push(Action::OpenDefinition { schema: s.schema.clone(), name: s.name.clone(), kind: "sequence".into() });
                }
            }
        });
    }
}
