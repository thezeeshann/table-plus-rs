//! Structure and index editors. They never touch the database: they turn the user's edits
//! into `DdlOp`s, which the table tab previews and applies in one transaction.

use eframe::egui::{self, RichText, Ui};
use table_plus_core::model::{ColumnInfo, TableStructure};
use table_plus_core::pg::ddl::DdlOp;

use crate::theme::{c32, Colors};
use crate::widgets;

pub const PG_TYPES: &[&str] = &[
    "integer", "bigint", "smallint", "serial", "bigserial", "text", "varchar(255)", "char(1)", "boolean", "numeric(10,2)",
    "real", "double precision", "date", "time", "timestamp", "timestamptz", "interval", "uuid", "json", "jsonb", "bytea",
    "inet", "text[]", "integer[]",
];

const INDEX_METHODS: &[&str] = &["btree", "hash", "gin", "gist", "brin"];

struct ColEdit {
    orig: Option<ColumnInfo>,
    name: String,
    ty: String,
    nullable: bool,
    default: String,
    comment: String,
    drop: bool,
}

#[derive(Default)]
pub struct State {
    cols: Vec<ColEdit>,
    table_comment: String,
    orig_table_comment: String,
    synced: bool,
    idx_name: String,
    idx_cols: String,
    idx_method: usize,
    idx_unique: bool,
    idx_where: String,
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl State {
    /// Rebuilds the edit buffers from freshly loaded structure.
    pub fn reset(&mut self) {
        self.synced = false;
        self.cols.clear();
    }

    fn sync(&mut self, st: &TableStructure) {
        if self.synced {
            return;
        }
        self.cols = st
            .columns
            .iter()
            .map(|c| ColEdit {
                orig: Some(c.clone()),
                name: c.name.clone(),
                ty: c.format_type.clone(),
                nullable: c.nullable,
                default: c.default.clone().unwrap_or_default(),
                comment: c.comment.clone().unwrap_or_default(),
                drop: false,
            })
            .collect();
        self.table_comment = st.comment.clone().unwrap_or_default();
        self.orig_table_comment = self.table_comment.clone();
        self.synced = true;
    }

    /// DDL for everything that differs from the loaded structure.
    pub fn pending_ops(&self) -> Vec<DdlOp> {
        let mut ops = Vec::new();
        let mut renames = Vec::new();
        for c in &self.cols {
            match &c.orig {
                None => {
                    if !c.drop && !c.name.trim().is_empty() {
                        ops.push(DdlOp::AddColumn {
                            name: c.name.trim().into(),
                            data_type: if c.ty.trim().is_empty() { "text".into() } else { c.ty.trim().into() },
                            nullable: c.nullable,
                            default: opt(&c.default),
                            comment: opt(&c.comment),
                        });
                    }
                }
                Some(o) => {
                    if c.drop {
                        ops.push(DdlOp::DropColumn { name: o.name.clone() });
                        continue;
                    }
                    if c.ty.trim() != o.format_type && !c.ty.trim().is_empty() {
                        ops.push(DdlOp::ChangeType { name: o.name.clone(), data_type: c.ty.trim().into(), using: None });
                    }
                    if c.nullable != o.nullable {
                        ops.push(DdlOp::SetNullable { name: o.name.clone(), nullable: c.nullable });
                    }
                    if c.default.trim() != o.default.clone().unwrap_or_default().trim() {
                        ops.push(DdlOp::SetDefault { name: o.name.clone(), default: opt(&c.default) });
                    }
                    if c.comment.trim() != o.comment.clone().unwrap_or_default().trim() {
                        ops.push(DdlOp::SetComment { name: o.name.clone(), comment: opt(&c.comment) });
                    }
                    if c.name.trim() != o.name && !c.name.trim().is_empty() {
                        renames.push(DdlOp::RenameColumn { name: o.name.clone(), new_name: c.name.trim().into() });
                    }
                }
            }
        }
        if self.table_comment.trim() != self.orig_table_comment.trim() {
            ops.push(DdlOp::SetTableComment { comment: opt(&self.table_comment) });
        }
        ops.extend(renames);
        ops
    }
}

/// Type box with a menu of common types.
pub fn type_picker(ui: &mut Ui, text: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add(egui::TextEdit::singleline(text).desired_width(width).font(egui::TextStyle::Monospace));
        ui.menu_button("…", |ui| {
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for t in PG_TYPES {
                    if ui.button(*t).clicked() {
                        *text = (*t).to_string();
                        ui.close();
                    }
                }
            });
        });
    });
}

pub fn show_structure(ui: &mut Ui, colors: &Colors, state: &mut State, st: &TableStructure, editable: bool) -> Option<Vec<DdlOp>> {
    state.sync(st);
    let mut result = None;
    let pending = state.pending_ops();
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            widgets::section_title(ui, colors, "Columns");
            if editable {
                if widgets::small_button(ui, "+ Column").clicked() {
                    state.cols.push(ColEdit { orig: None, name: String::new(), ty: "text".into(), nullable: true, default: String::new(), comment: String::new(), drop: false });
                }
                let label = if pending.is_empty() { "No changes".to_string() } else { format!("Review {} change(s)…", pending.len()) };
                if widgets::primary_button(ui, colors, &label, !pending.is_empty()).clicked() {
                    result = Some(pending.clone());
                }
                if !pending.is_empty() && widgets::small_button(ui, "Discard").clicked() {
                    state.reset();
                }
            }
        });
        ui.add_space(4.0);
        egui::Grid::new("columns_grid").num_columns(8).striped(true).spacing([10.0, 6.0]).min_row_height(24.0).show(ui, |ui| {
            for h in ["#", "Name", "Type", "Null", "Default", "Comment", "Keys", ""] {
                ui.label(RichText::new(h).size(11.0).color(c32(&colors.text_dim)));
            }
            ui.end_row();
            let mut remove: Option<usize> = None;
            for (i, c) in state.cols.iter_mut().enumerate() {
                let dropped = c.drop;
                ui.label(RichText::new(match &c.orig { Some(o) => o.position.to_string(), None => "new".into() }).size(11.0).color(c32(&colors.text_dim)));
                ui.add_enabled_ui(editable && !dropped, |ui| ui.add_sized([150.0, 22.0], egui::TextEdit::singleline(&mut c.name)));
                ui.add_enabled_ui(editable && !dropped, |ui| type_picker(ui, &mut c.ty, 130.0));
                ui.add_enabled(editable && !dropped, egui::Checkbox::without_text(&mut c.nullable));
                ui.add_enabled_ui(editable && !dropped, |ui| ui.add_sized([170.0, 22.0], egui::TextEdit::singleline(&mut c.default).font(egui::TextStyle::Monospace)));
                ui.add_enabled_ui(editable && !dropped, |ui| ui.add_sized([200.0, 22.0], egui::TextEdit::singleline(&mut c.comment)));
                ui.horizontal(|ui| {
                    if let Some(o) = &c.orig {
                        if o.is_primary_key {
                            widgets::pill(ui, "PK", c32(&colors.key_color));
                        }
                        if let Some(fk) = &o.foreign_key {
                            widgets::pill(ui, &format!("FK → {}", fk.parent_table), c32(&colors.accent));
                        }
                        if o.is_identity {
                            widgets::pill(ui, "identity", c32(&colors.text_dim));
                        }
                        if o.is_generated {
                            widgets::pill(ui, "generated", c32(&colors.text_dim));
                        }
                    }
                });
                if editable {
                    let label = if c.orig.is_none() { "Remove" } else if dropped { "Undo" } else { "Drop" };
                    if widgets::small_button(ui, label).clicked() {
                        if c.orig.is_none() {
                            remove = Some(i);
                        } else {
                            c.drop = !c.drop;
                        }
                    }
                } else {
                    ui.label("");
                }
                ui.end_row();
            }
            if let Some(i) = remove {
                state.cols.remove(i);
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new("Table comment").size(11.0).color(c32(&colors.text_dim)));
            ui.add_enabled(editable, egui::TextEdit::singleline(&mut state.table_comment).desired_width(420.0));
        });

        if !st.foreign_keys.is_empty() {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                widgets::section_title(ui, colors, "Foreign keys");
            });
            for fk in &st.foreign_keys {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&fk.constraint_name).strong());
                    ui.label(format!(
                        "({}) → {}.{} ({})  on update {}, on delete {}",
                        fk.child_columns.join(", "), fk.parent_schema, fk.parent_table, fk.parent_columns.join(", "),
                        fk.on_update.clone().unwrap_or_else(|| "NO ACTION".into()), fk.on_delete.clone().unwrap_or_else(|| "NO ACTION".into())
                    ));
                    if editable && widgets::small_button(ui, "Drop").clicked() {
                        result = Some(vec![DdlOp::DropConstraint { name: fk.constraint_name.clone() }]);
                    }
                });
            }
        }
        if !st.checks.is_empty() {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                widgets::section_title(ui, colors, "Check constraints");
            });
            for ck in &st.checks {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&ck.name).strong());
                    ui.label(RichText::new(&ck.condition).monospace());
                    if editable && widgets::small_button(ui, "Drop").clicked() {
                        result = Some(vec![DdlOp::DropConstraint { name: ck.name.clone() }]);
                    }
                });
            }
        }
        if !st.triggers.is_empty() {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                widgets::section_title(ui, colors, "Triggers");
            });
            for t in &st.triggers {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&t.name).strong());
                    ui.label(format!("{} {}", t.timing, t.event));
                    ui.label(RichText::new(&t.statement).monospace().color(c32(&colors.text_dim)));
                });
            }
        }
        ui.add_space(16.0);
    });
    result
}

pub fn show_indexes(ui: &mut Ui, colors: &Colors, state: &mut State, st: &TableStructure, editable: bool) -> Option<Vec<DdlOp>> {
    let mut result = None;
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(6.0);
        egui::Grid::new("index_grid").num_columns(7).striped(true).spacing([14.0, 6.0]).min_row_height(24.0).show(ui, |ui| {
            for h in ["Name", "Method", "Columns", "Condition", "Size", "", ""] {
                ui.label(RichText::new(h).size(11.0).color(c32(&colors.text_dim)));
            }
            ui.end_row();
            for ix in &st.indexes {
                ui.label(RichText::new(&ix.name).strong());
                ui.label(&ix.algorithm);
                ui.label(RichText::new(ix.columns.join(", ")).monospace());
                ui.label(RichText::new(ix.condition.clone().unwrap_or_default()).monospace());
                ui.label(ix.size.map(widgets::fmt_bytes).unwrap_or_default());
                ui.horizontal(|ui| {
                    if ix.is_primary {
                        widgets::pill(ui, "PRIMARY", c32(&colors.key_color));
                    } else if ix.is_unique {
                        widgets::pill(ui, "UNIQUE", c32(&colors.accent));
                    }
                });
                if editable && !ix.is_primary && widgets::small_button(ui, "Drop").clicked() {
                    result = Some(vec![DdlOp::DropIndex { name: ix.name.clone() }]);
                }
                ui.end_row();
            }
        });
        if editable {
            ui.add_space(14.0);
            widgets::section_title(ui, colors, "Create index");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut state.idx_name).hint_text("index name").desired_width(160.0));
                ui.add(egui::TextEdit::singleline(&mut state.idx_cols).hint_text("columns, comma separated").desired_width(220.0));
                egui::ComboBox::from_id_salt("idx_method").selected_text(INDEX_METHODS[state.idx_method]).show_ui(ui, |ui| {
                    for (i, m) in INDEX_METHODS.iter().enumerate() {
                        ui.selectable_value(&mut state.idx_method, i, *m);
                    }
                });
                ui.checkbox(&mut state.idx_unique, "Unique");
                ui.add(egui::TextEdit::singleline(&mut state.idx_where).hint_text("WHERE (optional)").desired_width(160.0));
                let cols: Vec<String> = state.idx_cols.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let ok = !state.idx_name.trim().is_empty() && !cols.is_empty();
                if widgets::primary_button(ui, colors, "Create…", ok).clicked() {
                    result = Some(vec![DdlOp::AddIndex {
                        name: state.idx_name.trim().into(),
                        columns: cols,
                        unique: state.idx_unique,
                        method: Some(INDEX_METHODS[state.idx_method].into()),
                        condition: opt(&state.idx_where),
                    }]);
                }
            });
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            position: 1, name: name.into(), data_type: ty.into(), format_type: ty.into(), nullable, default: None, comment: None,
            is_primary_key: false, foreign_key: None, is_identity: false, is_generated: false, generation_expression: None,
            character_maximum_length: None, numeric_precision: None, numeric_scale: None, enum_values: None,
        }
    }

    fn structure() -> TableStructure {
        TableStructure {
            schema: "public".into(), table: "t".into(), kind: "table".into(), columns: vec![col("a", "text", true), col("b", "integer", false)],
            primary_key: vec![], foreign_keys: vec![], indexes: vec![], checks: vec![], triggers: vec![], comment: None,
        }
    }

    #[test]
    fn no_edits_means_no_ops() {
        let mut s = State::default();
        s.sync(&structure());
        assert!(s.pending_ops().is_empty());
    }

    #[test]
    fn edits_turn_into_ops_with_renames_last() {
        let mut s = State::default();
        s.sync(&structure());
        s.cols[0].name = "renamed".into();
        s.cols[0].ty = "varchar(10)".into();
        s.cols[1].nullable = true;
        s.cols[1].drop = false;
        s.cols.push(ColEdit { orig: None, name: "c".into(), ty: "uuid".into(), nullable: false, default: "gen_random_uuid()".into(), comment: String::new(), drop: false });
        let ops = s.pending_ops();
        assert!(matches!(ops[0], DdlOp::ChangeType { .. }));
        assert!(matches!(ops[1], DdlOp::SetNullable { nullable: true, .. }));
        assert!(matches!(ops[2], DdlOp::AddColumn { .. }));
        assert!(matches!(ops.last().unwrap(), DdlOp::RenameColumn { .. }));
    }

    #[test]
    fn dropping_a_column_skips_its_other_edits() {
        let mut s = State::default();
        s.sync(&structure());
        s.cols[0].ty = "uuid".into();
        s.cols[0].drop = true;
        let ops = s.pending_ops();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], DdlOp::DropColumn { .. }));
    }
}
