//! SQL editor tab: highlighting, autocomplete, run current / run all, results grid, export.

use std::time::Instant;

use eframe::egui::{self, Align, Key, Layout, Modifiers, RichText, Ui};
use table_plus_core::export::{export_rows, ExportRequest};
use table_plus_core::model::{FavoriteQuery, QueryResult};

use crate::app::err_msg;
use crate::editor::{self, CompletionSchema};
use crate::grid::{self, CellVal, ColumnMeta, GridModel, GridState, GridView};
use crate::task::{poll_slot, Task};
use crate::theme::c32;
use crate::widgets;
use crate::workspace::TabCx;

struct ResultModel<'a>(&'a QueryResult);

impl GridModel for ResultModel<'_> {
    fn row_count(&self) -> usize {
        self.0.rows.len()
    }

    fn cell(&self, row: usize, col: usize) -> CellVal<'_> {
        match &self.0.rows[row][col] {
            None => CellVal::Null,
            Some(t) => CellVal::Text(t),
        }
    }
}

pub struct QueryTab {
    pub title: String,
    pub sql: String,
    results: Vec<QueryResult>,
    active_result: usize,
    task: Option<Task<Vec<QueryResult>>>,
    started: Option<Instant>,
    last_elapsed_ms: Option<f64>,
    cursor: usize,
    grid: GridState,
    save_name: Option<String>,
    editor_id: egui::Id,
    seq: u64,
}

impl QueryTab {
    pub fn new(seq: u64, title: String, sql: String) -> Self {
        Self {
            title, sql, results: vec![], active_result: 0, task: None, started: None, last_elapsed_ms: None, cursor: 0,
            grid: GridState::default(), save_name: None, editor_id: egui::Id::new(("sql_editor", seq)), seq,
        }
    }

    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    pub fn run(&mut self, cx: &mut TabCx, all: bool) {
        if self.task.is_some() {
            return;
        }
        let sql = if all { self.sql.clone() } else { editor::current_statement(&self.sql, self.cursor) };
        if sql.trim().is_empty() {
            return;
        }
        let s = cx.session.clone();
        self.started = Some(Instant::now());
        self.task = Some(cx.env.sp.spawn(async move { s.run_query(&sql, all, None, true).await.map_err(err_msg) }));
    }

    fn cancel(&mut self, cx: &mut TabCx) {
        let s = cx.session.clone();
        cx.env.sp.detach(async move {
            let _ = s.cancel_running().await;
        });
    }

    pub fn poll(&mut self, cx: &mut TabCx) {
        if let Some(res) = poll_slot(&mut self.task) {
            self.last_elapsed_ms = self.started.take().map(|t| t.elapsed().as_secs_f64() * 1000.0);
            match res {
                Ok(r) => {
                    self.results = r;
                    self.active_result = 0;
                    self.grid.clear();
                    if self.results.iter().any(|r| r.error.is_none() && r.rows_affected.is_some() && r.columns.is_empty()) {
                        // DDL/DML may have changed the schema.
                        cx.actions.push(crate::workspace::Action::RefreshListing);
                    }
                }
                Err(e) => cx.env.err(e.to_string()),
            }
        }
    }

    fn export(&self, cx: &mut TabCx, format: &str, ext: &str) {
        let Some(r) = self.results.get(self.active_result) else { return };
        let Some(path) = rfd::FileDialog::new().set_file_name(format!("result.{ext}")).save_file() else { return };
        let columns: Vec<String> = r.columns.iter().map(|c| c.name.clone()).collect();
        let req = ExportRequest { path: path.to_str().unwrap_or("result"), format, columns: &columns, rows: &r.rows, table: None, header: true };
        match export_rows(&req) {
            Ok(n) => cx.env.ok(format!("Exported {n} rows to {}", path.display())),
            Err(e) => cx.env.err(e.payload().one_line()),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut TabCx, schema: &CompletionSchema) {
        self.poll(cx);
        let ctx = ui.ctx().clone();
        let colors = cx.env.colors.clone();
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if cx.active && !cx.modal_open {
            let (mut run, mut all) = (false, false);
            ctx.input_mut(|i| {
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter) { all = true }
                if i.consume_key(Modifiers::COMMAND, Key::Enter) { run = true }
            });
            if all {
                self.run(cx, true);
            } else if run {
                self.run(cx, false);
            }
        }

        // toolbar
        egui::Frame::new().fill(c32(&colors.app_bg)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let r = ui.max_rect();
            ui.painter().line_segment([r.left_bottom() + egui::vec2(-10.0, 6.0), r.right_bottom() + egui::vec2(10.0, 6.0)], egui::Stroke::new(1.0, c32(&colors.control_border)));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let running = self.task.is_some();
                let btn = |ui: &mut Ui, text: &str, enabled: bool, tip: &str| ui.add_enabled(enabled, egui::Button::new(text).min_size(egui::vec2(0.0, 26.0))).on_hover_text(tip).clicked();
                if btn(ui, "▶  Run Current", !running, "Run the statement under the cursor (Cmd+Enter)") {
                    self.run(cx, false);
                }
                if btn(ui, "Run All", !running, "Run every statement (Cmd+Shift+Enter)") {
                    self.run(cx, true);
                }
                if btn(ui, "Stop", running, "Cancel the running query") {
                    self.cancel(cx);
                }
                let (sep, _) = ui.allocate_exact_size(egui::vec2(9.0, 20.0), egui::Sense::hover());
                ui.painter().line_segment([sep.center_top(), sep.center_bottom()], egui::Stroke::new(1.0, c32(&colors.control_border)));
                if btn(ui, "★  Save", true, "Save as a favourite query") {
                    self.save_name = Some(self.title.clone());
                }
                if btn(ui, "Save file", true, "Write the SQL to a .sql file") {
                    if let Some(p) = rfd::FileDialog::new().set_file_name("query.sql").save_file() {
                        match std::fs::write(&p, &self.sql) {
                            Ok(()) => cx.env.ok(format!("Saved {}", p.display())),
                            Err(e) => cx.env.err(e.to_string()),
                        }
                    }
                }
                if btn(ui, "Open file", true, "Load a .sql file into this tab") {
                    if let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).pick_file() {
                        match std::fs::read_to_string(&p) {
                            Ok(t) => {
                                self.sql = t;
                                self.title = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(self.title.clone());
                            }
                            Err(e) => cx.env.err(e.to_string()),
                        }
                    }
                }
                if running {
                    ui.spinner();
                    if let Some(t) = self.started {
                        ui.label(widgets::dim(ui, format!("{:.1}s", t.elapsed().as_secs_f32())));
                    }
                }
            });
        });

        // results (resizable bottom panel)
        egui::Panel::bottom(egui::Id::new(("q_results", self.seq)))
            .resizable(true)
            .default_size(300.0)
            .size_range(90.0..=900.0)
            .frame(egui::Frame::new().fill(c32(&colors.content_bg)))
            .show(ui, |ui| self.results_ui(ui, cx));

        // editor
        egui::Frame::new().fill(c32(&colors.content_bg)).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let out = editor::editor(ui, &colors, self.editor_id, &mut self.sql, cx.env.settings.editor_font_size as f32, schema);
                self.cursor = out.cursor_byte;
            });
        });

        self.save_dialog(&ctx, cx);
    }

    fn results_ui(&mut self, ui: &mut Ui, cx: &mut TabCx) {
        let colors = cx.env.colors.clone();
        if self.results.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(widgets::dim(ui, if self.busy() { "Running…" } else { "Run a query to see results here." }));
            });
            return;
        }
        if self.results.len() > 1 {
            ui.horizontal(|ui| {
                for (i, r) in self.results.iter().enumerate() {
                    let label = if r.error.is_some() { format!("#{} ✗", i + 1) } else { format!("#{}", i + 1) };
                    if ui.selectable_label(self.active_result == i, label).clicked() {
                        self.active_result = i;
                        self.grid.clear();
                    }
                }
            });
        }
        let idx = self.active_result.min(self.results.len() - 1);
        let r = &self.results[idx];
        if let Some(e) = &r.error {
            egui::Frame::new().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.label(RichText::new(&e.message).color(c32(&colors.danger_text)).strong());
                if let Some(code) = &e.code {
                    ui.label(widgets::dim(ui, format!("SQLSTATE {code}")));
                }
                if let Some(d) = &e.detail {
                    ui.label(format!("Detail: {d}"));
                }
                if let Some(h) = &e.hint {
                    ui.label(RichText::new(format!("Hint: {h}")).color(c32(&colors.success_text)));
                }
                if let Some(p) = e.position {
                    let line = r.statement.chars().take(p as usize).filter(|c| *c == '\n').count() + 1;
                    ui.label(widgets::dim(ui, format!("At character {p} (line {line}) of:")));
                    ui.label(RichText::new(r.statement.trim()).monospace());
                }
            });
            return;
        }
        if r.columns.is_empty() {
            egui::Frame::new().inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                let cmd = r.command.clone().unwrap_or_default();
                ui.label(RichText::new(format!("{cmd} OK")).color(c32(&colors.success_text)).strong());
                ui.label(widgets::dim(ui, format!("{} row(s) affected · {}", r.rows_affected.unwrap_or(0), widgets::fmt_ms(r.duration_ms))));
            });
            return;
        }
        let metas: Vec<ColumnMeta> = r.columns.iter().map(|c| ColumnMeta { name: c.name.clone(), type_name: c.type_name.clone(), is_pk: false, is_fk: false }).collect();
        let (n_rows, truncated, duration) = (r.rows.len(), r.truncated, r.duration_ms);
        let footer_h = 30.0;
        let grid_h = (ui.available_height() - footer_h - 8.0).max(40.0);
        let width = ui.available_width();
        ui.allocate_ui(egui::vec2(width, grid_h), |ui| {
            let r = &self.results[idx];
            let view = GridView { id: egui::Id::new(("qgrid", self.seq, idx)), columns: &metas, sort: &[], editable: false, first_row_number: 1, colors: &colors };
            let out = grid::show(ui, &view, &ResultModel(r), &mut self.grid);
            for a in out.actions {
                if let grid::ContextAction::Copy(t) = a {
                    ui.ctx().copy_text(t);
                }
            }
        });
        egui::Frame::new().fill(c32(&colors.content_bg)).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(footer_h - 8.0);
            ui.horizontal(|ui| {
                let trunc = if truncated { "  ·  truncated" } else { "" };
                ui.label(widgets::dim(ui, format!("{} rows  ·  {}{trunc}", widgets::fmt_count(n_rows as i64), widgets::fmt_ms(duration))));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("Export…", |ui| {
                        for (label, fmt, ext) in [("CSV", "csv", "csv"), ("JSON", "json", "json"), ("SQL INSERTs", "sql", "sql"), ("Markdown", "markdown", "md")] {
                            if ui.button(label).clicked() {
                                self.export(cx, fmt, ext);
                                ui.close();
                            }
                        }
                    });
                });
            });
        });
    }

    fn save_dialog(&mut self, ctx: &egui::Context, cx: &mut TabCx) {
        let Some(name) = &mut self.save_name else { return };
        let colors = cx.env.colors.clone();
        let (mut save, mut close) = (false, false);
        let modal = widgets::modal(ctx, &colors, "save_query").show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading("Save query");
            ui.add_space(8.0);
            let r = ui.add(egui::TextEdit::singleline(name).hint_text("Name").desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                save = true;
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &colors, "Save", !name.trim().is_empty()).clicked() {
                    save = true;
                }
                if widgets::button(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if save {
            let fav = FavoriteQuery {
                id: String::new(),
                name: name.trim().to_string(),
                sql: self.sql.clone(),
                connection_id: Some(cx.session.info.connection_id.clone()),
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            match cx.env.storage.save_favorite(fav) {
                Ok(_) => {
                    cx.env.ok("Query saved");
                    cx.actions.push(crate::workspace::Action::RefreshFavorites);
                }
                Err(e) => cx.env.err(e.payload().one_line()),
            }
            self.save_name = None;
        } else if close {
            self.save_name = None;
        }
    }
}
