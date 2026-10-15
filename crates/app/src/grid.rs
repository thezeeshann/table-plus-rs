//! The data grid: virtualised rows, resizable columns, row/cell selection, inline editing,
//! sort headers and a cell context menu. Used for table data (editable) and query results.

use std::collections::BTreeSet;

use eframe::egui::{self, Align, Color32, CornerRadius, FontId, Key, Modifiers, RichText, Sense, Stroke, StrokeKind, Ui};
use egui_extras::{Column, TableBuilder};
use table_plus_core::model::{CellValue, SortSpec};

use crate::theme::{c32, c32a, mono, Colors};

pub enum CellVal<'a> {
    Null,
    Text(&'a str),
    Default,
    Expr(&'a str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Normal,
    Inserted,
    Deleted,
}

pub trait GridModel {
    fn row_count(&self) -> usize;
    fn cell(&self, row: usize, col: usize) -> CellVal<'_>;
    fn cell_modified(&self, _row: usize, _col: usize) -> bool {
        false
    }
    fn row_state(&self, _row: usize) -> RowState {
        RowState::Normal
    }
}

pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub is_pk: bool,
    pub is_fk: bool,
}

pub struct Editing {
    pub row: usize,
    pub col: usize,
    pub buf: String,
    fresh: bool,
}

#[derive(Default)]
pub struct GridState {
    pub selected: BTreeSet<usize>,
    pub anchor: Option<usize>,
    pub cell: Option<(usize, usize)>,
    pub editing: Option<Editing>,
    scroll_to: Option<usize>,
    widths: Vec<f32>,
}

impl GridState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn select_only(&mut self, row: usize) {
        self.selected.clear();
        self.selected.insert(row);
        self.anchor = Some(row);
    }

    pub fn begin_edit(&mut self, row: usize, col: usize, initial: String) {
        self.cell = Some((row, col));
        self.editing = Some(Editing { row, col, buf: initial, fresh: true });
    }
}

pub enum ContextAction {
    SetNull(usize, usize),
    SetDefault(usize, usize),
    Copy(String),
    FilterBy(usize, Option<String>),
    FollowFk(usize, usize),
    DuplicateRows,
    DeleteRows,
}

#[derive(Default)]
pub struct GridOutput {
    /// (column, shift held)
    pub sort: Option<(usize, bool)>,
    pub edited: Option<(usize, usize, CellValue)>,
    pub actions: Vec<ContextAction>,
}

pub struct GridView<'a> {
    pub id: egui::Id,
    pub columns: &'a [ColumnMeta],
    pub sort: &'a [SortSpec],
    pub editable: bool,
    /// Number shown for the first row (page offset + 1).
    pub first_row_number: u64,
    pub colors: &'a Colors,
}

const ROW_H: f32 = 24.0;

fn one_line(s: &str) -> String {
    if s.contains('\n') || s.contains('\r') {
        s.replace("\r\n", "¶").replace(['\n', '\r'], "¶")
    } else {
        s.to_string()
    }
}

pub fn show(ui: &mut Ui, view: &GridView, model: &dyn GridModel, st: &mut GridState) -> GridOutput {
    let mut out = GridOutput::default();
    let colors = view.colors;
    let n = model.row_count();
    let ncols = view.columns.len();
    let font = mono(12.5);

    // Keyboard: arrows move the focused cell; Enter edits; ⌘C copies.
    if st.editing.is_none() && ncols > 0 && n > 0 && ui.memory(|m| m.focused().is_none()) {
        let (mut dr, mut dc) = (0i64, 0i64);
        let mut enter = false;
        let mut copy = false;
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) { dr = 1 }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) { dr = -1 }
            if i.consume_key(Modifiers::NONE, Key::ArrowRight) { dc = 1 }
            if i.consume_key(Modifiers::NONE, Key::ArrowLeft) { dc = -1 }
            if i.consume_key(Modifiers::NONE, Key::Enter) { enter = true }
            if i.consume_key(Modifiers::COMMAND, Key::C) { copy = true }
        });
        let cur = st.cell.unwrap_or((0, 0));
        if dr != 0 || dc != 0 {
            let r = (cur.0 as i64 + dr).clamp(0, n as i64 - 1) as usize;
            let c = (cur.1 as i64 + dc).clamp(0, ncols as i64 - 1) as usize;
            st.cell = Some((r, c));
            if dr != 0 {
                st.select_only(r);
                st.scroll_to = Some(r);
            }
        }
        if enter && view.editable {
            if let Some((r, c)) = st.cell {
                let initial = match model.cell(r, c) {
                    CellVal::Text(t) | CellVal::Expr(t) => t.to_string(),
                    _ => String::new(),
                };
                st.begin_edit(r, c, initial);
            }
        }
        if copy {
            if let Some((r, c)) = st.cell {
                if let CellVal::Text(t) | CellVal::Expr(t) = model.cell(r, c) {
                    ui.ctx().copy_text(t.to_string());
                }
            }
        }
    }

    if st.widths.len() != ncols {
        st.widths = (0..ncols)
            .map(|c| {
                let mut chars = view.columns[c].name.chars().count() + 4;
                for r in 0..n.min(60) {
                    if let CellVal::Text(t) | CellVal::Expr(t) = model.cell(r, c) {
                        chars = chars.max(t.lines().next().map(|l| l.chars().count()).unwrap_or(0) + 2);
                    }
                }
                (chars as f32 * 7.9 + 16.0).clamp(80.0, 380.0)
            })
            .collect();
    }
    let mut table = TableBuilder::new(ui)
        .id_salt(view.id)
        .striped(false)
        .resizable(true)
        .vscroll(true)
        .auto_shrink([false, false])
        .cell_layout(egui::Layout::left_to_right(Align::Center))
        .column(Column::exact(46.0));
    for w in &st.widths {
        table = table.column(Column::initial(*w).at_least(44.0).clip(true));
    }
    if let Some(r) = st.scroll_to.take() {
        table = table.scroll_to_row(r, Some(Align::Center));
    }

    let header_bg = c32(&colors.grid_header);
    let text_col = c32(&colors.text);
    let dim = c32(&colors.text_dim);

    table
        .header(28.0, |mut header| {
            header.col(|ui| {
                ui.painter().rect_filled(ui.max_rect(), 0.0, header_bg);
            });
            for (ci, c) in view.columns.iter().enumerate() {
                header.col(|ui| {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, header_bg);
                    ui.painter().line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0, c32(&colors.control_border)));
                    let resp = ui.interact(rect, ui.id().with(("hdr", ci)), Sense::click());
                    let mut x = rect.left() + 8.0;
                    let cy = rect.center().y;
                    if c.is_pk {
                        let g = ui.painter().text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, "PK", FontId::proportional(9.5), c32(&colors.key_color));
                        x += g.width() + 5.0;
                    }
                    let g = ui.painter().text(egui::pos2(x, cy), egui::Align2::LEFT_CENTER, &c.name, FontId::proportional(12.5), text_col);
                    x += g.width() + 6.0;
                    if !c.type_name.is_empty() && x + 20.0 < rect.right() {
                        ui.painter().text(egui::pos2(x, cy + 1.0), egui::Align2::LEFT_CENTER, &c.type_name, FontId::proportional(10.0), dim);
                    }
                    if let Some(pos) = view.sort.iter().position(|s| s.column == c.name) {
                        let arrow = if view.sort[pos].direction == "desc" { "▼" } else { "▲" };
                        ui.painter().text(egui::pos2(rect.right() - 8.0, cy), egui::Align2::RIGHT_CENTER, arrow, FontId::proportional(10.0), c32(&colors.accent));
                    }
                    if resp.clicked() {
                        out.sort = Some((ci, ui.input(|i| i.modifiers.shift)));
                    }
                    resp.on_hover_text(format!("{} {}", c.name, c.type_name).trim().to_string());
                });
            }
        })
        .body(|body| {
            body.rows(ROW_H, n, |mut row| {
                let ri = row.index();
                let state = model.row_state(ri);
                let selected = st.selected.contains(&ri);
                let base = if state == RowState::Inserted {
                    Some(c32(&colors.row_inserted))
                } else if state == RowState::Deleted {
                    Some(c32(&colors.row_deleted))
                } else if selected {
                    Some(c32(&colors.selection))
                } else if ri % 2 == 1 {
                    Some(c32(&colors.grid_even))
                } else {
                    None
                };
                let row_fill = base.map(|c| if selected && state != RowState::Normal { c.gamma_multiply(0.85) } else { c });

                // row number
                row.col(|ui| {
                    let rect = ui.max_rect();
                    if let Some(f) = row_fill {
                        ui.painter().rect_filled(rect, 0.0, f);
                    }
                    let resp = ui.interact(rect, ui.id().with(("rn", ri)), Sense::click());
                    ui.painter().text(
                        egui::pos2(rect.right() - 8.0, rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        (view.first_row_number + ri as u64).to_string(),
                        FontId::proportional(11.0),
                        dim,
                    );
                    if resp.clicked() {
                        let m = ui.input(|i| i.modifiers);
                        select_row(st, ri, m);
                    }
                });

                for ci in 0..ncols {
                    row.col(|ui| {
                        let rect = ui.max_rect();
                        let modified = model.cell_modified(ri, ci);
                        let fill = if modified && state == RowState::Normal { Some(c32(&colors.row_modified)) } else { row_fill };
                        if let Some(f) = fill {
                            ui.painter().rect_filled(rect, 0.0, f);
                        }
                        ui.painter().line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, Color32::from_black_alpha(48)));

                        let is_editing = matches!(&st.editing, Some(e) if e.row == ri && e.col == ci);
                        if is_editing {
                            let e = st.editing.as_mut().unwrap();
                            let resp = ui.put(
                                rect.shrink2(egui::vec2(1.0, 1.0)),
                                egui::TextEdit::singleline(&mut e.buf).font(font.clone()).desired_width(rect.width()).margin(egui::Margin::symmetric(5, 3)),
                            );
                            if e.fresh {
                                resp.request_focus();
                                e.fresh = false;
                            }
                            let esc = ui.input(|i| i.key_pressed(Key::Escape));
                            let enter = resp.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if esc {
                                st.editing = None;
                            } else if enter || (resp.lost_focus() && !esc) {
                                let e = st.editing.take().unwrap();
                                out.edited = Some((e.row, e.col, CellValue::Text { value: e.buf }));
                            }
                            return;
                        }

                        let resp = ui.interact(rect, ui.id().with(("cell", ri, ci)), Sense::click());
                        let (text, color, italic): (String, Color32, bool) = match model.cell(ri, ci) {
                            CellVal::Null => ("NULL".into(), c32a(&colors.text, 0.38), true),
                            CellVal::Default => ("DEFAULT".into(), c32(&colors.accent), true),
                            CellVal::Expr(e) => (format!("= {e}"), c32(&colors.accent), false),
                            CellVal::Text(t) => (one_line(t), text_col, false),
                        };
                        let pos = egui::pos2(rect.left() + 7.0, rect.center().y);
                        let galley = ui.painter().layout_no_wrap(text.clone(), if italic { FontId::proportional(11.5) } else { font.clone() }, color);
                        ui.painter().with_clip_rect(rect.shrink2(egui::vec2(0.0, 0.0))).galley(pos - egui::vec2(0.0, galley.size().y / 2.0), galley, color);
                        if state == RowState::Deleted {
                            ui.painter().line_segment([egui::pos2(rect.left() + 4.0, rect.center().y), egui::pos2(rect.right() - 4.0, rect.center().y)], Stroke::new(1.0, Color32::from_white_alpha(120)));
                        }
                        if st.cell == Some((ri, ci)) {
                            ui.painter().rect_stroke(rect.shrink(0.5), CornerRadius::ZERO, Stroke::new(1.5, c32(&colors.selection_border)), StrokeKind::Inside);
                        }
                        let is_fk = view.columns[ci].is_fk;
                        if is_fk && matches!(model.cell(ri, ci), CellVal::Text(_)) {
                            let arrow = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::vec2(16.0, 16.0));
                            let ar = ui.interact(arrow, ui.id().with(("fk", ri, ci)), Sense::click());
                            ui.painter().text(arrow.center(), egui::Align2::CENTER_CENTER, "→", FontId::proportional(13.0), if ar.hovered() { Color32::WHITE } else { c32(&colors.accent) });
                            if ar.clicked() {
                                out.actions.push(ContextAction::FollowFk(ri, ci));
                            }
                        }
                        if resp.clicked() {
                            st.cell = Some((ri, ci));
                            let m = ui.input(|i| i.modifiers);
                            if !(m.command || m.shift) {
                                st.select_only(ri);
                            } else {
                                select_row(st, ri, m);
                            }
                        }
                        if resp.double_clicked() && view.editable && state != RowState::Deleted {
                            let initial = match model.cell(ri, ci) {
                                CellVal::Text(t) | CellVal::Expr(t) => t.to_string(),
                                _ => String::new(),
                            };
                            st.begin_edit(ri, ci, initial);
                        }
                        resp.context_menu(|ui| {
                            st.cell = Some((ri, ci));
                            if !st.selected.contains(&ri) {
                                st.select_only(ri);
                            }
                            let value = match model.cell(ri, ci) {
                                CellVal::Text(t) | CellVal::Expr(t) => Some(t.to_string()),
                                _ => None,
                            };
                            if ui.button("Copy value").clicked() {
                                out.actions.push(ContextAction::Copy(value.clone().unwrap_or_else(|| "NULL".into())));
                                ui.close();
                            }
                            if ui.button("Filter by this value").clicked() {
                                out.actions.push(ContextAction::FilterBy(ci, value.clone()));
                                ui.close();
                            }
                            if view.editable {
                                ui.separator();
                                if ui.button("Set NULL").clicked() {
                                    out.actions.push(ContextAction::SetNull(ri, ci));
                                    ui.close();
                                }
                                if ui.button("Set DEFAULT").clicked() {
                                    out.actions.push(ContextAction::SetDefault(ri, ci));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Duplicate row(s)").clicked() {
                                    out.actions.push(ContextAction::DuplicateRows);
                                    ui.close();
                                }
                                if ui.button(RichText::new("Delete row(s)").color(c32(&colors.danger_text))).clicked() {
                                    out.actions.push(ContextAction::DeleteRows);
                                    ui.close();
                                }
                            }
                        });
                    });
                }
            });
        });
    out
}

fn select_row(st: &mut GridState, row: usize, m: Modifiers) {
    if m.shift {
        let a = st.anchor.unwrap_or(row);
        let (lo, hi) = (a.min(row), a.max(row));
        st.selected = (lo..=hi).collect();
    } else if m.command {
        if !st.selected.remove(&row) {
            st.selected.insert(row);
        }
        st.anchor = Some(row);
    } else {
        st.select_only(row);
    }
}
