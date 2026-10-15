//! A table (or view) tab: Data / Structure / Index / DDL, filters, pagination and staged edits.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use eframe::egui::{self, Align, Key, Layout, Modifiers, RichText, Ui};
use table_plus_core::model::*;
use table_plus_core::pg::ddl::{ddl_sql, DdlOp};
use table_plus_core::pg::sqlgen::generate_change_sql;
use table_plus_core::Session;

use crate::app::err_msg;
use crate::editor;
use crate::grid::{self, CellVal, ColumnMeta, ContextAction, GridModel, GridState, GridView, RowState};
use crate::structure;
use crate::task::{poll_slot, Task};
use crate::theme::c32;
use crate::widgets;
use crate::workspace::{Action, TabCx};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Data,
    Structure,
    Index,
    Ddl,
}

const FILTER_OPS: &[(&str, &str)] = &[
    ("eq", "="), ("neq", "≠"), ("gt", ">"), ("gte", "≥"), ("lt", "<"), ("lte", "≤"), ("contains", "contains"),
    ("not_contains", "not contains"), ("starts_with", "starts with"), ("ends_with", "ends with"), ("like", "LIKE"),
    ("not_like", "NOT LIKE"), ("in", "IN (a, b)"), ("not_in", "NOT IN"), ("is_null", "IS NULL"), ("is_not_null", "IS NOT NULL"),
    ("between", "between"), ("raw", "raw SQL"),
];

fn needs_value(op: &str) -> bool {
    !matches!(op, "is_null" | "is_not_null")
}

#[derive(Default)]
struct Staged {
    edits: BTreeMap<(usize, usize), CellValue>,
    inserts: Vec<Vec<CellValue>>,
    deletes: BTreeSet<usize>,
}

impl Staged {
    fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.inserts.is_empty() && self.deletes.is_empty()
    }

    fn count(&self) -> usize {
        let rows: BTreeSet<usize> = self.edits.keys().map(|k| k.0).collect();
        rows.len() + self.inserts.len() + self.deletes.len()
    }
}

struct Model<'a> {
    rows: &'a [Vec<Option<String>>],
    staged: &'a Staged,
}

fn cv_to_cell(v: &CellValue) -> CellVal<'_> {
    match v {
        CellValue::Null => CellVal::Null,
        CellValue::Text { value } => CellVal::Text(value),
        CellValue::Default => CellVal::Default,
        CellValue::Expr { value } => CellVal::Expr(value),
    }
}

impl GridModel for Model<'_> {
    fn row_count(&self) -> usize {
        self.rows.len() + self.staged.inserts.len()
    }

    fn cell(&self, row: usize, col: usize) -> CellVal<'_> {
        let n = self.rows.len();
        if row >= n {
            return cv_to_cell(&self.staged.inserts[row - n][col]);
        }
        if let Some(v) = self.staged.edits.get(&(row, col)) {
            return cv_to_cell(v);
        }
        match &self.rows[row][col] {
            None => CellVal::Null,
            Some(t) => CellVal::Text(t),
        }
    }

    fn cell_modified(&self, row: usize, col: usize) -> bool {
        self.staged.edits.contains_key(&(row, col))
    }

    fn row_state(&self, row: usize) -> RowState {
        if row >= self.rows.len() {
            RowState::Inserted
        } else if self.staged.deletes.contains(&row) {
            RowState::Deleted
        } else {
            RowState::Normal
        }
    }
}

enum Dialog {
    /// Preview of grid edits; Commit applies them.
    Data(Vec<String>),
    /// Preview of structure/index DDL; Apply runs it.
    Ddl(Vec<DdlOp>, Vec<String>),
    ConfirmDiscard,
}

pub struct TableTab {
    pub schema: String,
    pub name: String,
    pub kind: String,
    view: View,
    data: Option<FetchRowsResponse>,
    fetch: Option<Task<FetchRowsResponse>>,
    structure: Option<TableStructure>,
    structure_task: Option<Task<TableStructure>>,
    ddl: Option<String>,
    ddl_task: Option<Task<String>>,
    error: Option<String>,
    sort: Vec<SortSpec>,
    filters: Vec<FilterSpec>,
    filters_open: bool,
    quick: String,
    offset: u64,
    limit: u32,
    staged: Staged,
    grid: GridState,
    dialog: Option<Dialog>,
    commit: Option<Task<Vec<String>>>,
    ddl_apply: Option<Task<Vec<String>>>,
    structure_ui: structure::State,
    rename_to: Option<String>,
}

impl TableTab {
    pub fn new(cx: &mut TabCx, schema: String, name: String, kind: String, filter: Option<FilterSpec>) -> Self {
        let mut t = Self {
            schema, name, kind,
            view: View::Data,
            data: None, fetch: None, structure: None, structure_task: None, ddl: None, ddl_task: None, error: None,
            sort: vec![], filters: filter.into_iter().collect(), filters_open: false, quick: String::new(), offset: 0,
            limit: cx.env.settings.default_limit.max(10), staged: Staged::default(), grid: GridState::default(),
            dialog: None, commit: None, ddl_apply: None, structure_ui: structure::State::default(), rename_to: None,
        };
        t.filters_open = !t.filters.is_empty();
        t.load(cx);
        t
    }

    /// True while rows are being fetched (drives the toolbar loading pill).
    pub fn busy(&self) -> bool {
        self.fetch.is_some() || self.commit.is_some() || self.ddl_apply.is_some()
    }

    pub fn has_staged(&self) -> bool {
        !self.staged.is_empty()
    }

    pub fn editable(&self) -> bool {
        matches!(self.kind.as_str(), "table" | "partitioned")
    }

    pub fn reload(&mut self, cx: &mut TabCx) {
        self.load(cx);
    }

    fn active_filters(&self) -> Vec<FilterSpec> {
        self.filters
            .iter()
            .filter(|f| f.enabled && (f.op == "raw" || !f.column.is_empty()) && (!needs_value(&f.op) || f.value.as_deref().map(|v| !v.is_empty()).unwrap_or(false)))
            .cloned()
            .collect()
    }

    fn load(&mut self, cx: &mut TabCx) {
        let s: Arc<Session> = cx.session.clone();
        let req = FetchRowsRequest {
            schema: self.schema.clone(),
            table: self.name.clone(),
            kind: self.kind.clone(),
            limit: self.limit,
            offset: self.offset,
            sort: self.sort.clone(),
            filters: self.active_filters(),
            quick_filter: (!self.quick.trim().is_empty()).then(|| self.quick.trim().to_string()),
            count_mode: "auto".into(),
        };
        let threshold = cx.env.settings.estimate_count_threshold;
        self.error = None;
        self.fetch = Some(cx.env.sp.spawn(async move { s.fetch_rows(&req, threshold).await.map_err(err_msg) }));
    }

    fn load_structure(&mut self, cx: &mut TabCx) {
        let s = cx.session.clone();
        let (schema, name, kind) = (self.schema.clone(), self.name.clone(), self.kind.clone());
        self.structure_task = Some(cx.env.sp.spawn(async move { s.table_structure(&schema, &name, &kind).await.map_err(err_msg) }));
    }

    fn load_ddl(&mut self, cx: &mut TabCx) {
        let s = cx.session.clone();
        let (schema, name, kind) = (self.schema.clone(), self.name.clone(), self.kind.clone());
        self.ddl_task = Some(cx.env.sp.spawn(async move { s.object_ddl(&schema, &name, &kind).await.map_err(err_msg) }));
    }

    pub fn poll(&mut self, cx: &mut TabCx) {
        if let Some(res) = poll_slot(&mut self.fetch) {
            match res {
                Ok(d) => {
                    self.data = Some(d);
                    self.grid.clear();
                    if self.structure.is_none() && self.structure_task.is_none() {
                        self.load_structure(cx);
                    }
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        if let Some(res) = poll_slot(&mut self.structure_task) {
            match res {
                Ok(s) => {
                    self.structure = Some(s);
                    self.structure_ui.reset();
                }
                Err(e) => cx.env.err(format!("Structure: {e}")),
            }
        }
        if let Some(res) = poll_slot(&mut self.ddl_task) {
            match res {
                Ok(s) => self.ddl = Some(s),
                Err(e) => self.ddl = Some(format!("-- {e}")),
            }
        }
        if let Some(res) = poll_slot(&mut self.commit) {
            match res {
                Ok(stmts) => {
                    cx.env.ok(format!("Committed {} statement(s)", stmts.len()));
                    self.staged = Staged::default();
                    self.load(cx);
                }
                Err(e) => cx.env.err(format!("Commit failed, nothing was changed: {e}")),
            }
        }
        if let Some(res) = poll_slot(&mut self.ddl_apply) {
            match res {
                Ok(stmts) => {
                    cx.env.ok(format!("Applied {} statement(s)", stmts.len()));
                    if let Some(n) = self.rename_to.take() {
                        self.name = n;
                    }
                    self.structure = None;
                    self.structure_ui.reset();
                    self.load_structure(cx);
                    self.ddl = None;
                    self.load(cx);
                    cx.actions.push(Action::RefreshListing);
                }
                Err(e) => {
                    self.rename_to = None;
                    cx.env.err(format!("Failed, nothing was changed: {e}"));
                }
            }
        }
    }

    // ---- staging helpers

    fn n_rows(&self) -> usize {
        self.data.as_ref().map(|d| d.rows.len()).unwrap_or(0)
    }

    fn stage_cell(&mut self, row: usize, col: usize, value: CellValue) {
        let n = self.n_rows();
        if row >= n {
            if let Some(r) = self.staged.inserts.get_mut(row - n) {
                r[col] = value;
            }
            return;
        }
        let original = self.data.as_ref().and_then(|d| d.rows[row][col].clone());
        let same = match (&value, &original) {
            (CellValue::Text { value: v }, Some(o)) => v == o,
            (CellValue::Null, None) => true,
            _ => false,
        };
        if same {
            self.staged.edits.remove(&(row, col));
        } else {
            self.staged.edits.insert((row, col), value);
        }
    }

    fn add_row(&mut self) {
        let ncols = self.data.as_ref().map(|d| d.columns.len()).unwrap_or(0);
        if ncols == 0 {
            return;
        }
        self.staged.inserts.push(vec![CellValue::Default; ncols]);
        let row = self.n_rows() + self.staged.inserts.len() - 1;
        self.grid.select_only(row);
        self.grid.cell = Some((row, 0));
    }

    fn duplicate_rows(&mut self) {
        let Some(d) = &self.data else { return };
        let auto: Vec<bool> = d
            .columns
            .iter()
            .map(|c| {
                self.structure
                    .as_ref()
                    .and_then(|s| s.columns.iter().find(|x| x.name == c.name))
                    .map(|x| x.is_identity || x.default.as_deref().map(|d| d.starts_with("nextval")).unwrap_or(false))
                    .unwrap_or(false)
            })
            .collect();
        let mut new_rows = vec![];
        for &r in &self.grid.selected {
            if r < d.rows.len() {
                new_rows.push(
                    d.rows[r]
                        .iter()
                        .enumerate()
                        .map(|(c, v)| match (auto[c], self.staged.edits.get(&(r, c)), v) {
                            (true, ..) => CellValue::Default,
                            (_, Some(e), _) => e.clone(),
                            (_, None, Some(t)) => CellValue::Text { value: t.clone() },
                            (_, None, None) => CellValue::Null,
                        })
                        .collect::<Vec<_>>(),
                );
            }
        }
        self.staged.inserts.extend(new_rows);
    }

    fn delete_rows(&mut self) {
        let n = self.n_rows();
        let sel: Vec<usize> = self.grid.selected.iter().copied().collect();
        let mut remove_inserts: Vec<usize> = sel.iter().filter(|&&r| r >= n).map(|r| r - n).collect();
        remove_inserts.sort_unstable_by(|a, b| b.cmp(a));
        for i in remove_inserts {
            if i < self.staged.inserts.len() {
                self.staged.inserts.remove(i);
            }
        }
        for r in sel.into_iter().filter(|&r| r < n) {
            if !self.staged.deletes.remove(&r) {
                self.staged.deletes.insert(r);
            }
        }
        self.grid.selected.clear();
    }

    fn build_request(&self) -> Result<ApplyChangesRequest, String> {
        let d = self.data.as_ref().ok_or("no data loaded")?;
        let st = self.structure.as_ref().ok_or("table structure is still loading; try again in a moment")?;
        let cols: Vec<&str> = d.columns.iter().map(|c| c.name.as_str()).collect();
        let identity = |r: usize| RowIdentity {
            key_values: st
                .primary_key
                .iter()
                .filter_map(|k| cols.iter().position(|c| c == k).map(|i| (k.clone(), d.rows[r][i].clone())))
                .collect(),
            ctid: d.ctids.as_ref().and_then(|c| c.get(r).cloned()),
        };
        let mut changes = vec![];
        let edited_rows: BTreeSet<usize> = self.staged.edits.keys().map(|k| k.0).collect();
        for r in edited_rows {
            if self.staged.deletes.contains(&r) {
                continue;
            }
            let values: BTreeMap<String, CellValue> =
                self.staged.edits.iter().filter(|((er, _), _)| *er == r).map(|((_, c), v)| (cols[*c].to_string(), v.clone())).collect();
            changes.push(RowChange::Update { identity: identity(r), values });
        }
        for &r in &self.staged.deletes {
            changes.push(RowChange::Delete { identity: identity(r) });
        }
        for row in &self.staged.inserts {
            let values: BTreeMap<String, CellValue> = row
                .iter()
                .enumerate()
                .filter(|(_, v)| !matches!(v, CellValue::Default))
                .map(|(c, v)| (cols[c].to_string(), v.clone()))
                .collect();
            changes.push(RowChange::Insert { values });
        }
        Ok(ApplyChangesRequest { schema: self.schema.clone(), table: self.name.clone(), changes, primary_key: st.primary_key.clone() })
    }

    fn preview(&mut self, cx: &mut TabCx) {
        match self.build_request().and_then(|r| generate_change_sql(&r.schema, &r.table, &r.primary_key, &r.changes).map_err(|e| e.payload().one_line())) {
            Ok(stmts) if stmts.is_empty() => cx.env.ok("No changes"),
            Ok(stmts) => self.dialog = Some(Dialog::Data(stmts)),
            Err(e) => cx.env.err(e),
        }
    }

    fn commit_now(&mut self, cx: &mut TabCx) {
        if self.commit.is_some() || self.staged.is_empty() {
            return;
        }
        match self.build_request() {
            Ok(req) => {
                let s = cx.session.clone();
                self.commit = Some(cx.env.sp.spawn(async move { s.apply_changes(&req).await.map_err(err_msg) }));
            }
            Err(e) => cx.env.err(e),
        }
    }

    fn review_ddl(&mut self, ops: Vec<DdlOp>) {
        let stmts = ddl_sql(&self.schema, &self.name, &ops);
        if !stmts.is_empty() {
            self.dialog = Some(Dialog::Ddl(ops, stmts));
        }
    }

    fn discard(&mut self) {
        self.staged = Staged::default();
        self.grid.clear();
    }

    fn apply_filter_from_cell(&mut self, cx: &mut TabCx, col: usize, value: Option<String>) {
        let Some(d) = &self.data else { return };
        let name = d.columns[col].name.clone();
        self.filters.push(match value {
            Some(v) => FilterSpec { column: name, op: "eq".into(), value: Some(v), value2: None, enabled: true },
            None => FilterSpec { column: name, op: "is_null".into(), value: None, value2: None, enabled: true },
        });
        self.filters_open = true;
        self.offset = 0;
        self.load(cx);
    }

    fn handle_action(&mut self, cx: &mut TabCx, ctx: &egui::Context, a: ContextAction) {
        match a {
            ContextAction::SetNull(r, c) => self.stage_cell(r, c, CellValue::Null),
            ContextAction::SetDefault(r, c) => self.stage_cell(r, c, CellValue::Default),
            ContextAction::Copy(t) => ctx.copy_text(t),
            ContextAction::FilterBy(c, v) => self.apply_filter_from_cell(cx, c, v),
            ContextAction::DuplicateRows => self.duplicate_rows(),
            ContextAction::DeleteRows => self.delete_rows(),
            ContextAction::FollowFk(r, c) => {
                let (Some(d), Some(st)) = (&self.data, &self.structure) else { return };
                let col = &d.columns[c].name;
                let Some(fk) = st.columns.iter().find(|x| &x.name == col).and_then(|x| x.foreign_key.clone()) else { return };
                let Some(value) = d.rows.get(r).and_then(|row| row[c].clone()) else { return };
                let idx = fk.child_columns.iter().position(|x| x == col).unwrap_or(0);
                let parent_col = fk.parent_columns.get(idx).cloned().unwrap_or_default();
                cx.actions.push(Action::OpenTable {
                    schema: fk.parent_schema,
                    name: fk.parent_table,
                    kind: "table".into(),
                    filter: Some(FilterSpec { column: parent_col, op: "eq".into(), value: Some(value), value2: None, enabled: true }),
                    pin: true,
                });
            }
        }
    }

    fn shortcuts(&mut self, cx: &mut TabCx, ctx: &egui::Context) {
        if !cx.active || cx.modal_open || self.grid.editing.is_some() {
            return;
        }
        let editable = self.editable() && self.view == View::Data;
        let (mut commit, mut preview, mut discard, mut add, mut dup, mut del, mut reload) = (false, false, false, false, false, false, false);
        let mut view = None;
        let typing = ctx.memory(|m| m.focused().is_some());
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::S) { commit = true }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::P) { preview = true }
            if i.consume_key(Modifiers::COMMAND, Key::Escape) { discard = true }
            if i.consume_key(Modifiers::COMMAND, Key::I) { add = true }
            if i.consume_key(Modifiers::COMMAND, Key::R) { reload = true }
            if !typing {
                if i.consume_key(Modifiers::COMMAND, Key::D) { dup = true }
                if i.consume_key(Modifiers::COMMAND, Key::Backspace) { del = true }
            }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Num1) { view = Some(View::Data) }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Num2) { view = Some(View::Structure) }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Num3) { view = Some(View::Index) }
        });
        if let Some(v) = view {
            self.set_view(cx, v);
        }
        if reload {
            self.load(cx);
        }
        if editable {
            if commit {
                self.commit_now(cx);
            }
            if preview {
                self.preview(cx);
            }
            if discard && !self.staged.is_empty() {
                self.discard();
            }
            if add {
                self.add_row();
            }
            if dup {
                self.duplicate_rows();
            }
            if del {
                self.delete_rows();
            }
        }
    }

    pub fn script_view(&mut self, cx: &mut TabCx, view: &str) {
        let v = match view {
            "structure" => View::Structure,
            "index" => View::Index,
            "ddl" => View::Ddl,
            _ => View::Data,
        };
        self.set_view(cx, v);
    }

    pub fn script_edit(&mut self, row: usize, col: usize, text: &str) {
        self.stage_cell(row, col, CellValue::Text { value: text.to_string() });
    }

    pub fn script_preview(&mut self, cx: &mut TabCx) {
        self.preview(cx);
    }

    pub fn script_close_dialog(&mut self) {
        self.dialog = None;
    }

    fn set_view(&mut self, cx: &mut TabCx, v: View) {
        self.view = v;
        if v == View::Ddl && self.ddl.is_none() && self.ddl_task.is_none() {
            self.load_ddl(cx);
        }
        if matches!(v, View::Structure | View::Index) && self.structure.is_none() && self.structure_task.is_none() {
            self.load_structure(cx);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut TabCx) {
        self.poll(cx);
        let ctx = ui.ctx().clone();
        self.shortcuts(cx, &ctx);
        let colors = cx.env.colors.clone();
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // ---- bottom bar
        egui::Panel::bottom(egui::Id::new(("tbl_bottom", &self.schema, &self.name))).exact_size(38.0).frame(
            egui::Frame::new().fill(c32(&colors.sidebar_bg)).inner_margin(egui::Margin::symmetric(10, 6)),
        ).show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut v = self.view;
                let tabs: &[(View, &str)] = &[(View::Data, "Data"), (View::Structure, "Structure"), (View::Index, "Index"), (View::Ddl, "DDL")];
                if widgets::segmented(ui, &colors, &mut v, tabs) {
                    self.set_view(cx, v);
                }
                if self.view == View::Data {
                    if self.editable() && widgets::small_button(ui, "+ Row").clicked() {
                        self.add_row();
                    }
                    let active = self.active_filters().len();
                    let label = if active > 0 { format!("Filters ({active})") } else { "Filters".to_string() };
                    if widgets::small_button(ui, &label).clicked() {
                        self.filters_open = !self.filters_open;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.view == View::Data {
                        self.pager(ui, cx);
                    }
                });
            });
        });

        // ---- staged strip + filters
        if self.view == View::Data {
            let strip = egui::Frame::new().fill(c32(&colors.sidebar_bg)).inner_margin(egui::Margin::symmetric(10, 6));
            {
                strip.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::TextEdit::singleline(&mut self.quick).hint_text("Quick filter (all columns)…").desired_width(240.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.offset = 0;
                            self.load(cx);
                        }
                        if widgets::small_button(ui, "Reload").clicked() {
                            self.load(cx);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if !self.staged.is_empty() {
                                if widgets::primary_button(ui, &colors, &format!("Commit ({})", self.staged.count()), self.commit.is_none()).clicked() {
                                    self.commit_now(cx);
                                }
                                if widgets::button(ui, "Preview SQL").clicked() {
                                    self.preview(cx);
                                }
                                if widgets::button(ui, "Discard").clicked() {
                                    self.dialog = Some(Dialog::ConfirmDiscard);
                                }
                                ui.label(RichText::new(format!("{} pending", self.staged.count())).color(c32(&colors.key_color)));
                            }
                        });
                    });
                    if self.filters_open {
                        self.filter_rows(ui, cx);
                    }
                });
            }
        }

        // ---- main area
        let editable = self.editable();
        egui::Frame::new().fill(c32(&colors.content_bg)).show(ui, |ui| match self.view {
            View::Data => self.data_view(ui, cx, &ctx),
            View::Structure => match self.structure.clone() {
                Some(st) => {
                    if let Some(ops) = structure::show_structure(ui, &colors, &mut self.structure_ui, &st, editable) {
                        self.review_ddl(ops);
                    }
                }
                None => loading(ui, "Loading structure…"),
            },
            View::Index => match self.structure.clone() {
                Some(st) => {
                    if let Some(ops) = structure::show_indexes(ui, &colors, &mut self.structure_ui, &st, editable) {
                        self.review_ddl(ops);
                    }
                }
                None => loading(ui, "Loading indexes…"),
            },
            View::Ddl => match &self.ddl {
                Some(text) => {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        if widgets::small_button(ui, "Copy").clicked() {
                            ctx.copy_text(text.clone());
                        }
                    });
                    ui.add_space(4.0);
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
                        editor::sql_view(ui, &colors, text, ui.available_height() - 12.0);
                    });
                }
                None => loading(ui, "Loading definition…"),
            },
        });

        self.dialogs(&ctx, cx);
    }

    fn pager(&mut self, ui: &mut Ui, cx: &mut TabCx) {
        let Some(d) = &self.data else { return };
        let (n_rows, total, estimate, duration) = (d.rows.len(), d.total, d.total_is_estimate, d.duration_ms);
        let first = self.offset + 1;
        let last = self.offset + n_rows as u64;
        let can_next = match total {
            Some(t) if !estimate => (last as i64) < t,
            _ => n_rows as u32 >= self.limit,
        };
        if ui.add_enabled(can_next, egui::Button::new("›").min_size(egui::vec2(24.0, 22.0))).clicked() {
            self.offset += self.limit as u64;
            self.load(cx);
        }
        if ui.add_enabled(self.offset > 0, egui::Button::new("‹").min_size(egui::vec2(24.0, 22.0))).clicked() {
            self.offset = self.offset.saturating_sub(self.limit as u64);
            self.load(cx);
        }
        let total = match total {
            Some(t) if estimate => format!("~{}", widgets::fmt_count(t)),
            Some(t) => widgets::fmt_count(t),
            None => "?".into(),
        };
        let range = if n_rows == 0 { "0 rows".to_string() } else { format!("{}–{} of {total} rows", widgets::fmt_count(first as i64), widgets::fmt_count(last as i64)) };
        ui.label(RichText::new(format!("{range}  ·  {}", widgets::fmt_ms(duration))).size(12.0).color(c32(&cx.env.colors.text_dim)));
    }

    fn filter_rows(&mut self, ui: &mut Ui, cx: &mut TabCx) {
        let cols: Vec<String> = self.data.as_ref().map(|d| d.columns.iter().map(|c| c.name.clone()).collect()).unwrap_or_default();
        let mut remove = None;
        let mut apply = false;
        for (i, f) in self.filters.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.enabled, "");
                if f.op != "raw" {
                    egui::ComboBox::from_id_salt(("fcol", i)).width(150.0).selected_text(if f.column.is_empty() { "column".to_string() } else { f.column.clone() }).show_ui(ui, |ui| {
                        for c in &cols {
                            ui.selectable_value(&mut f.column, c.clone(), c);
                        }
                    });
                }
                let label = FILTER_OPS.iter().find(|o| o.0 == f.op).map(|o| o.1).unwrap_or("=");
                egui::ComboBox::from_id_salt(("fop", i)).width(110.0).selected_text(label).show_ui(ui, |ui| {
                    for (id, l) in FILTER_OPS {
                        ui.selectable_value(&mut f.op, (*id).to_string(), *l);
                    }
                });
                if needs_value(&f.op) {
                    let mut v = f.value.clone().unwrap_or_default();
                    let r = ui.add(egui::TextEdit::singleline(&mut v).hint_text(if f.op == "raw" { "e.g. created_at > now() - interval '1 day'" } else { "value" }).desired_width(260.0));
                    f.value = Some(v);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        apply = true;
                    }
                    if f.op == "between" {
                        let mut v2 = f.value2.clone().unwrap_or_default();
                        ui.add(egui::TextEdit::singleline(&mut v2).hint_text("and").desired_width(120.0));
                        f.value2 = Some(v2);
                    }
                }
                if widgets::small_button(ui, "×").clicked() {
                    remove = Some(i);
                }
            });
        }
        ui.horizontal(|ui| {
            if widgets::small_button(ui, "+ Filter").clicked() {
                self.filters.push(FilterSpec { column: String::new(), op: "eq".into(), value: None, value2: None, enabled: true });
            }
            if widgets::small_button(ui, "+ Raw SQL").clicked() {
                self.filters.push(FilterSpec { column: String::new(), op: "raw".into(), value: None, value2: None, enabled: true });
            }
            if widgets::primary_button(ui, &cx.env.colors, "Apply", true).clicked() {
                apply = true;
            }
        });
        if let Some(i) = remove {
            self.filters.remove(i);
            apply = true;
        }
        if apply {
            self.offset = 0;
            self.load(cx);
        }
    }

    fn data_view(&mut self, ui: &mut Ui, cx: &mut TabCx, ctx: &egui::Context) {
        if let Some(err) = self.error.clone() {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(err).color(c32(&cx.env.colors.danger_text)));
            });
            return;
        }
        let Some(data) = &self.data else {
            loading(ui, "Loading…");
            return;
        };
        let colors = cx.env.colors.clone();
        let metas: Vec<ColumnMeta> = data
            .columns
            .iter()
            .map(|c| {
                let sc = self.structure.as_ref().and_then(|s| s.columns.iter().find(|x| x.name == c.name));
                ColumnMeta {
                    name: c.name.clone(),
                    type_name: sc.map(|x| x.format_type.clone()).unwrap_or_else(|| c.type_name.clone()),
                    is_pk: sc.map(|x| x.is_primary_key).unwrap_or(false),
                    is_fk: sc.map(|x| x.foreign_key.is_some()).unwrap_or(false),
                }
            })
            .collect();
        if data.rows.is_empty() && self.staged.inserts.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(widgets::dim(ui, if self.fetch.is_some() { "Loading…" } else { "No rows" })));
            return;
        }
        let model = Model { rows: &data.rows, staged: &self.staged };
        let view = GridView {
            id: egui::Id::new(("grid", &self.schema, &self.name)),
            columns: &metas,
            sort: &self.sort,
            editable: self.editable(),
            first_row_number: self.offset + 1,
            colors: &colors,
        };
        let out = grid::show(ui, &view, &model, &mut self.grid);
        if let Some((c, shift)) = out.sort {
            let name = metas[c].name.clone();
            let cur = self.sort.iter().position(|s| s.column == name);
            if !shift {
                let next = match cur.map(|i| self.sort[i].direction.as_str()) {
                    None => Some("asc"),
                    Some("asc") => Some("desc"),
                    _ => None,
                };
                self.sort.clear();
                if let Some(d) = next {
                    self.sort.push(SortSpec { column: name, direction: d.into() });
                }
            } else {
                match cur {
                    None => self.sort.push(SortSpec { column: name, direction: "asc".into() }),
                    Some(i) if self.sort[i].direction == "asc" => self.sort[i].direction = "desc".into(),
                    Some(i) => {
                        self.sort.remove(i);
                    }
                }
            }
            self.offset = 0;
            self.load(cx);
        }
        if let Some((r, c, v)) = out.edited {
            self.stage_cell(r, c, v);
        }
        for a in out.actions {
            self.handle_action(cx, ctx, a);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, cx: &mut TabCx) {
        let colors = cx.env.colors.clone();
        let Some(dialog) = &self.dialog else { return };
        let mut close = false;
        let mut run = false;
        let (title, stmts, confirm_label): (&str, Vec<String>, &str) = match dialog {
            Dialog::Data(s) => ("Commit changes", s.clone(), "Commit"),
            Dialog::Ddl(_, s) => ("Apply changes", s.clone(), "Apply"),
            Dialog::ConfirmDiscard => ("Discard changes?", vec![], "Discard"),
        };
        let modal = widgets::modal(ctx, &colors, "table_dialog").show(ctx, |ui| {
            ui.set_width(620.0);
            ui.heading(title);
            ui.add_space(8.0);
            if stmts.is_empty() {
                ui.label(format!("{} pending change(s) will be thrown away.", self.staged.count()));
            } else {
                ui.label(widgets::dim(ui, format!("{} statement(s), run in one transaction:", stmts.len())));
                ui.add_space(4.0);
                editor::sql_view(ui, &colors, &stmts.iter().map(|s| format!("{s};")).collect::<Vec<_>>().join("\n"), 320.0);
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &colors, confirm_label, true).clicked() {
                    run = true;
                }
                if widgets::button(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        if modal.should_close() {
            close = true;
        }
        if run {
            match self.dialog.take() {
                Some(Dialog::Data(_)) => self.commit_now(cx),
                Some(Dialog::Ddl(ops, _)) => {
                    let s = cx.session.clone();
                    let (schema, table) = (self.schema.clone(), self.name.clone());
                    // Follow a rename (applied once the statement succeeds) so the tab keeps pointing at the table.
                    self.rename_to = ops.iter().find_map(|op| match op { DdlOp::RenameTable { new_name } => Some(new_name.clone()), _ => None });
                    self.ddl_apply = Some(cx.env.sp.spawn(async move { s.apply_ddl(&schema, &table, &ops).await.map_err(err_msg) }));
                }
                Some(Dialog::ConfirmDiscard) => self.discard(),
                None => {}
            }
        } else if close {
            self.dialog = None;
        }
    }
}

fn loading(ui: &mut Ui, text: &str) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.spinner();
        ui.add_space(6.0);
        ui.label(widgets::dim(ui, text));
    });
}
