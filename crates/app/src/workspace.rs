//! The connected window: toolbar, sidebar, tab strip, the active tab and dialogs.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, CornerRadius, Key, Layout, Modifiers, RichText, Sense, Stroke, StrokeKind, Ui};
use table_plus_core::model::*;
use table_plus_core::pg::ddl::{ddl_sql, DdlOp, NewColumn};
use table_plus_core::pg::sqlgen::{qualified, quote_ident};
use table_plus_core::Session;

use crate::app::{err_msg, Env};
use crate::editor::{self, CompletionSchema};
use crate::process_tab::ProcessTab;
use crate::query_tab::QueryTab;
use crate::sidebar;
use crate::structure;
use crate::table_tab::TableTab;
use crate::task::{poll_slot, Task};
use crate::theme::c32;
use crate::toolbar;
use crate::widgets;

pub enum Event {
    Disconnect,
    Settings,
    OpenDatabase(String),
}

/// A confirmed-before-running statement (drop, truncate, refresh).
#[derive(Clone)]
pub struct Destructive {
    pub title: String,
    pub message: String,
    pub sql: String,
    pub confirm: String,
    pub danger: bool,
    pub close_table: Option<(String, String)>,
}

impl Destructive {
    pub fn drop(schema: &str, name: &str, kind: &str) -> Self {
        let kw = match kind {
            "view" => "VIEW",
            "matview" => "MATERIALIZED VIEW",
            "foreign" => "FOREIGN TABLE",
            _ => "TABLE",
        };
        Self {
            title: format!("Drop {name}?"),
            message: format!("DROP {kw} {}. This cannot be undone.", qualified(schema, name)),
            sql: format!("DROP {kw} {}", qualified(schema, name)),
            confirm: "Drop".into(),
            danger: true,
            close_table: Some((schema.into(), name.into())),
        }
    }

    pub fn truncate(schema: &str, name: &str) -> Self {
        Self {
            title: format!("Truncate {name}?"),
            message: "Every row will be deleted. This cannot be undone.".into(),
            sql: format!("TRUNCATE TABLE {}", qualified(schema, name)),
            confirm: "Truncate".into(),
            danger: true,
            close_table: None,
        }
    }

    pub fn refresh_matview(schema: &str, name: &str) -> Self {
        Self {
            title: format!("Refresh {name}?"),
            message: "The materialized view will be recomputed.".into(),
            sql: format!("REFRESH MATERIALIZED VIEW {}", qualified(schema, name)),
            confirm: "Refresh".into(),
            danger: false,
            close_table: None,
        }
    }
}

pub enum Action {
    OpenTable { schema: String, name: String, kind: String, filter: Option<FilterSpec>, pin: bool },
    OpenDefinition { schema: String, name: String, kind: String },
    OpenQuery { title: String, sql: String },
    SwitchSchema(String),
    CreateTable,
    Rename { schema: String, name: String, kind: String },
    Destructive(Destructive),
    RefreshListing,
    RefreshFavorites,
    DeleteFavorite(String),
    ClearHistory,
}

/// What a tab needs from the workspace.
pub struct TabCx<'a> {
    pub env: &'a mut Env,
    pub session: &'a Arc<Session>,
    pub actions: &'a mut Vec<Action>,
    pub active: bool,
    pub modal_open: bool,
}

struct Definition {
    title: String,
    text: Option<String>,
    task: Option<Task<String>>,
}

enum Kind {
    Table(TableTab),
    Query(QueryTab),
    Processes(ProcessTab),
    Definition(Definition),
}

struct Tab {
    id: u64,
    pinned: bool,
    kind: Kind,
}

impl Tab {
    fn title(&self) -> String {
        match &self.kind {
            Kind::Table(t) => t.name.clone(),
            Kind::Query(q) => q.title.clone(),
            Kind::Processes(_) => "Processes".into(),
            Kind::Definition(d) => d.title.clone(),
        }
    }

    fn icon_kind(&self) -> crate::widgets::IconKind {
        use crate::widgets::IconKind;
        match &self.kind {
            Kind::Table(t) => IconKind::of_item(&t.kind),
            Kind::Query(_) => IconKind::Query,
            Kind::Processes(_) => IconKind::Processes,
            Kind::Definition(_) => IconKind::Function,
        }
    }

    fn busy(&self) -> bool {
        match &self.kind {
            Kind::Table(t) => t.busy(),
            Kind::Query(q) => q.busy(),
            Kind::Processes(p) => p.busy(),
            Kind::Definition(d) => d.task.is_some(),
        }
    }
}

struct Init {
    schemas: Vec<String>,
    schema: String,
    listing: SchemaListing,
    columns: BTreeMap<String, Vec<String>>,
}

struct CreateTable {
    name: String,
    cols: Vec<(String, String, bool, bool)>, // name, type, nullable, primary key
}

enum Dialog {
    CreateTable(CreateTable),
    Confirm(Destructive),
    Rename { schema: String, name: String, kind: String, text: String },
    OpenDb { dbs: Option<Vec<String>>, task: Option<Task<Vec<String>>> },
}

pub struct Workspace {
    session: Arc<Session>,
    schemas: Vec<String>,
    schema: String,
    listing: Option<SchemaListing>,
    completion: CompletionSchema,
    init: Option<Task<Init>>,
    refresh: Option<Task<(SchemaListing, BTreeMap<String, Vec<String>>)>>,
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    sidebar: sidebar::State,
    sidebar_open: bool,
    favorites: Vec<FavoriteQuery>,
    history: Vec<HistoryEntry>,
    history_at: Option<Instant>,
    dialog: Option<Dialog>,
    exec: Option<(Task<u64>, Option<(String, String)>)>,
    reconnect: Option<Task<Arc<Session>>>,
    /// Actions queued by tabs while they were being created.
    pub pending: Vec<Action>,
    open_db_event: Option<String>,
    open_after_exec: Option<String>,
}

fn matches_conn(f: &FavoriteQuery, id: &str) -> bool {
    f.connection_id.as_deref().map(|c| c == id).unwrap_or(true)
}

impl Workspace {
    pub fn new(env: &mut Env, session: Arc<Session>) -> Self {
        let s = session.clone();
        let wanted = s.config.last_opened_schema.clone();
        let init = env.sp.spawn(async move {
            let schemas = s.list_schemas().await.map_err(err_msg)?;
            let schema = wanted
                .filter(|w| schemas.contains(w))
                .or_else(|| schemas.iter().find(|x| *x == "public").cloned())
                .or_else(|| schemas.first().cloned())
                .unwrap_or_else(|| "public".into());
            let listing = s.list_items(&schema).await.map_err(err_msg)?;
            let columns = s.schema_columns(&schema).await.map_err(err_msg)?;
            Ok(Init { schemas, schema, listing, columns })
        });
        let favorites = env.storage.favorites().unwrap_or_default().into_iter().filter(|f| matches_conn(f, &session.info.connection_id)).collect();
        Self {
            session,
            schemas: vec![],
            schema: "public".into(),
            listing: None,
            completion: CompletionSchema::default(),
            init: Some(init),
            refresh: None,
            tabs: vec![],
            active: 0,
            next_id: 1,
            sidebar: sidebar::State::default(),
            sidebar_open: true,
            favorites,
            history: vec![],
            history_at: None,
            dialog: None,
            exec: None,
            reconnect: None,
            pending: vec![],
            open_db_event: None,
            open_after_exec: None,
        }
    }

    pub fn session_config(&self) -> ConnectionConfig {
        self.session.config.clone()
    }

    pub fn is_loading_listing(&self) -> bool {
        self.init.is_some() || self.refresh.is_some()
    }

    pub fn active_busy(&self) -> bool {
        self.tabs.get(self.active).map(|t| t.busy()).unwrap_or(false)
    }

    // ---- scripted-run hooks (see script.rs)

    pub fn script_open(&mut self, name: &str) {
        let found = self.listing.as_ref().and_then(|l| l.items.iter().find(|i| i.name == name)).cloned();
        if let Some(i) = found {
            self.pending.push(Action::OpenTable { schema: i.schema, name: i.name, kind: i.kind, filter: None, pin: true });
        }
    }

    pub fn script_view(&mut self, env: &mut Env, view: &str) {
        let session = self.session.clone();
        let mut sub = vec![];
        if let Some(Tab { kind: Kind::Table(t), .. }) = self.tabs.get_mut(self.active) {
            let mut cx = TabCx { env, session: &session, actions: &mut sub, active: true, modal_open: false };
            t.script_view(&mut cx, view);
        }
        self.pending.extend(sub);
    }

    pub fn script_query(&mut self, sql: &str) {
        self.open_query("SQL".into(), sql.to_string());
    }

    pub fn script_run(&mut self, env: &mut Env) {
        let session = self.session.clone();
        let mut sub = vec![];
        if let Some(Tab { kind: Kind::Query(q), .. }) = self.tabs.get_mut(self.active) {
            let mut cx = TabCx { env, session: &session, actions: &mut sub, active: true, modal_open: false };
            q.run(&mut cx, false);
        }
        self.pending.extend(sub);
    }

    pub fn script_edit(&mut self, row: usize, col: usize, text: &str) {
        if let Some(Tab { kind: Kind::Table(t), .. }) = self.tabs.get_mut(self.active) {
            t.script_edit(row, col, text);
        }
    }

    pub fn script_preview(&mut self, env: &mut Env) {
        let session = self.session.clone();
        let mut sub = vec![];
        if let Some(Tab { kind: Kind::Table(t), .. }) = self.tabs.get_mut(self.active) {
            let mut cx = TabCx { env, session: &session, actions: &mut sub, active: true, modal_open: false };
            t.script_preview(&mut cx);
        }
    }

    pub fn script_discard(&mut self) {
        if let Some(Tab { kind: Kind::Table(t), .. }) = self.tabs.get_mut(self.active) {
            t.script_close_dialog();
        }
    }

    fn start_refresh(&mut self, env: &mut Env) {
        if self.refresh.is_some() {
            return;
        }
        let (s, schema) = (self.session.clone(), self.schema.clone());
        self.refresh = Some(env.sp.spawn(async move {
            let listing = s.list_items(&schema).await.map_err(err_msg)?;
            let columns = s.schema_columns(&schema).await.map_err(err_msg)?;
            Ok((listing, columns))
        }));
    }

    fn poll(&mut self, env: &mut Env) {
        if let Some(r) = poll_slot(&mut self.init) {
            match r {
                Ok(i) => {
                    self.schemas = i.schemas;
                    self.schema = i.schema;
                    self.listing = Some(i.listing);
                    self.completion.columns = i.columns;
                }
                Err(e) => env.err(format!("Could not load the schema: {e}")),
            }
        }
        if let Some(r) = poll_slot(&mut self.refresh) {
            match r {
                Ok((l, c)) => {
                    self.listing = Some(l);
                    self.completion.columns = c;
                }
                Err(e) => env.err(format!("Refresh failed: {e}")),
            }
        }
        if let Some((task, close)) = &mut self.exec {
            if let Some(r) = task.poll() {
                let close = close.clone();
                self.exec = None;
                match r {
                    Ok(_) => {
                        env.ok("Done");
                        if let Some(name) = self.open_after_exec.take() {
                            self.pending.push(Action::OpenTable { schema: self.schema.clone(), name, kind: "table".into(), filter: None, pin: true });
                        }
                        if let Some((schema, name)) = close {
                            self.tabs.retain(|t| !matches!(&t.kind, Kind::Table(tt) if tt.schema == schema && tt.name == name));
                            self.active = self.active.min(self.tabs.len().saturating_sub(1));
                        }
                        self.start_refresh(env);
                    }
                    Err(e) => env.err(e.to_string()),
                }
            }
        }
        if let Some(r) = poll_slot(&mut self.reconnect) {
            match r {
                Ok(s) => {
                    self.session = s;
                    env.ok("Reconnected");
                    for t in &mut self.tabs {
                        if let Kind::Table(tt) = &mut t.kind {
                            let _ = tt;
                        }
                    }
                    self.start_refresh(env);
                }
                Err(e) => env.err(format!("Reconnect failed: {e}")),
            }
        }
        if self.sidebar.mode == sidebar::Mode::History && self.history_at.map(|t| t.elapsed() > Duration::from_secs(2)).unwrap_or(true) {
            self.history = env.storage.history(&self.session.info.connection_id, 200, None).unwrap_or_default();
            self.history_at = Some(Instant::now());
        }
    }

    fn new_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_tab(&mut self, kind: Kind, pinned: bool) {
        let id = self.new_id();
        self.tabs.push(Tab { id, pinned, kind });
        self.active = self.tabs.len() - 1;
    }

    fn open_query(&mut self, title: String, sql: String) {
        let id = self.next_id;
        self.add_tab(Kind::Query(QueryTab::new(id, title, sql)), true);
    }

    fn close_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            self.tabs.remove(i);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            } else if i < self.active {
                self.active -= 1;
            }
        }
    }

    fn open_table(&mut self, env: &mut Env, schema: String, name: String, kind: String, filter: Option<FilterSpec>, pin: bool) {
        if filter.is_none() {
            if let Some(i) = self.tabs.iter().position(|t| matches!(&t.kind, Kind::Table(tt) if tt.schema == schema && tt.name == name)) {
                self.active = i;
                if pin {
                    self.tabs[i].pinned = true;
                }
                return;
            }
        }
        let pinned = pin || filter.is_some();
        let session = self.session.clone();
        let mut sub = vec![];
        let tab = {
            let mut cx = TabCx { env, session: &session, actions: &mut sub, active: true, modal_open: false };
            TableTab::new(&mut cx, schema, name, kind, filter)
        };
        self.pending.extend(sub);
        // A fresh TableTab per table: a preview tab never shows the previous table's rows.
        let id = self.new_id();
        let tab = Tab { id, pinned, kind: Kind::Table(tab) };
        if !pinned {
            if let Some(i) = self.tabs.iter().position(|t| !t.pinned && matches!(&t.kind, Kind::Table(tt) if !tt.has_staged())) {
                self.tabs[i] = tab;
                self.active = i;
                return;
            }
        }
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    fn open_definition(&mut self, env: &mut Env, schema: String, name: String, kind: String) {
        let s = self.session.clone();
        let title = name.clone();
        if kind == "sequence" {
            let text = self
                .listing
                .as_ref()
                .and_then(|l| l.sequences.iter().find(|q| q.name == name && q.schema == schema))
                .map(|q| {
                    format!(
                        "-- sequence {}\nstart value : {}\nmin value   : {}\nmax value   : {}\nincrement   : {}\ncache       : {}\nlast value  : {}",
                        qualified(&schema, &name),
                        q.start_value.clone().unwrap_or_default(), q.min_value.clone().unwrap_or_default(), q.max_value.clone().unwrap_or_default(),
                        q.increment_by.clone().unwrap_or_default(), q.cache_size.clone().unwrap_or_default(), q.last_value.clone().unwrap_or_default()
                    )
                })
                .unwrap_or_default();
            self.add_tab(Kind::Definition(Definition { title, text: Some(text), task: None }), false);
            return;
        }
        let task = env.sp.spawn(async move { s.object_ddl(&schema, &name, &kind).await.map_err(err_msg) });
        self.add_tab(Kind::Definition(Definition { title, text: None, task: Some(task) }), false);
    }

    fn process_actions(&mut self, env: &mut Env, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::OpenTable { schema, name, kind, filter, pin } => self.open_table(env, schema, name, kind, filter, pin),
                Action::OpenDefinition { schema, name, kind } => self.open_definition(env, schema, name, kind),
                Action::OpenQuery { title, sql } => self.open_query(title, sql),
                Action::SwitchSchema(s) => {
                    self.schema = s;
                    self.listing = None;
                    self.refresh = None;
                    self.start_refresh(env);
                }
                Action::CreateTable => {
                    self.dialog = Some(Dialog::CreateTable(CreateTable {
                        name: String::new(),
                        cols: vec![("id".into(), "bigserial".into(), false, true), ("name".into(), "text".into(), true, false)],
                    }))
                }
                Action::Rename { schema, name, kind } => self.dialog = Some(Dialog::Rename { schema, text: name.clone(), name, kind }),
                Action::Destructive(d) => self.dialog = Some(Dialog::Confirm(d)),
                Action::RefreshListing => self.start_refresh(env),
                Action::RefreshFavorites => {
                    let id = self.session.info.connection_id.clone();
                    self.favorites = env.storage.favorites().unwrap_or_default().into_iter().filter(|f| matches_conn(f, &id)).collect();
                }
                Action::DeleteFavorite(id) => {
                    let _ = env.storage.delete_favorite(&id);
                    self.favorites.retain(|f| f.id != id);
                }
                Action::ClearHistory => {
                    let _ = env.storage.clear_history(&self.session.info.connection_id);
                    self.history.clear();
                }
            }
        }
    }

    fn start_exec(&mut self, env: &mut Env, sql: String, close: Option<(String, String)>) {
        let s = self.session.clone();
        self.exec = Some((env.sp.spawn(async move { s.execute(&sql).await.map_err(err_msg) }), close));
    }

    fn where_label(&self) -> (String, bool) {
        match self.tabs.get(self.active).map(|t| &t.kind) {
            Some(Kind::Table(t)) => (format!("{}.{}", t.schema, t.name), false),
            Some(Kind::Query(_)) => ("SQL".into(), true),
            Some(Kind::Processes(_)) => ("Processes".into(), false),
            Some(Kind::Definition(d)) => (d.title.clone(), false),
            None => (self.schema.clone(), false),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, env: &mut Env, modal_open: bool) -> Option<Event> {
        self.poll(env);
        let ctx = ui.ctx().clone();
        let mut actions: Vec<Action> = std::mem::take(&mut self.pending);
        let mut event = None;
        let colors = env.colors.clone();
        let dialog_open = self.dialog.is_some();
        let blocked = modal_open || dialog_open;

        // workspace shortcuts
        if !blocked {
            let (mut new_q, mut close, mut open_db, mut sidebar, mut procs, mut reconnect) = (false, false, false, false, false, false);
            ctx.input_mut(|i| {
                if i.consume_key(Modifiers::COMMAND, Key::T) { new_q = true }
                if i.consume_key(Modifiers::COMMAND, Key::W) { close = true }
                if i.consume_key(Modifiers::COMMAND, Key::K) { open_db = true }
                if i.consume_key(Modifiers::COMMAND, Key::Backslash) { sidebar = true }
                if i.consume_key(Modifiers::COMMAND, Key::Period) { procs = true }
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::R) { reconnect = true }
            });
            if new_q {
                self.open_query("SQL".into(), String::new());
            }
            if close && !self.tabs.is_empty() {
                self.close_tab(self.active);
            }
            if open_db {
                self.open_db_dialog(env);
            }
            if sidebar {
                self.sidebar_open = !self.sidebar_open;
            }
            if procs {
                self.open_processes(env);
            }
            if reconnect && self.reconnect.is_none() {
                self.begin_reconnect(env);
            }
        }

        // toolbar
        let (where_, is_query) = self.where_label();
        let busy = self.active_busy() || self.is_loading_listing() && self.tabs.is_empty();
        let info = &self.session.info;
        let tout = egui::Panel::top("toolbar").exact_size(52.0).frame(egui::Frame::new().fill(c32(&colors.app_bg))).show(ui, |ui| {
            toolbar::show(
                ui,
                &toolbar::Data {
                    colors: &colors,
                    color_tag: &info.color,
                    environment: &info.environment,
                    server: &info.server_version,
                    tls: info.tls.as_deref(),
                    database: &info.database,
                    user: &info.user,
                    where_: &where_,
                    is_query,
                    busy,
                    alive: self.session.is_alive(),
                    reconnecting: self.reconnect.is_some(),
                    sidebar_open: self.sidebar_open,
                    mode: self.sidebar.mode,
                },
            )
        });
        let t = tout.inner;
        if t.toggle_sidebar {
            self.sidebar_open = !self.sidebar_open;
        }
        if t.new_query {
            self.open_query("SQL".into(), String::new());
        }
        if t.processes {
            self.open_processes(env);
        }
        if t.settings {
            event = Some(Event::Settings);
        }
        if t.disconnect {
            event = Some(Event::Disconnect);
        }
        if t.open_database {
            self.open_db_dialog(env);
        }
        if t.reconnect && self.reconnect.is_none() {
            self.begin_reconnect(env);
        }
        if t.reload {
            self.reload_active(env, &mut actions);
        }
        if let Some(m) = t.mode {
            if self.sidebar_open && self.sidebar.mode == m {
                self.sidebar_open = false;
            } else {
                self.sidebar.mode = m;
                self.sidebar_open = true;
            }
        }
        if t.add_row {
            if let Some(Kind::Table(tt)) = self.tabs.get_mut(self.active).map(|t| &mut t.kind) {
                tt.request_add_row();
            }
        }

        // sidebar
        if self.sidebar_open {
            let active_item = match self.tabs.get(self.active).map(|t| &t.kind) {
                Some(Kind::Table(t)) => Some((t.schema.clone(), t.name.clone())),
                _ => None,
            };
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(250.0)
                .size_range(180.0..=520.0)
                .frame(egui::Frame::new().fill(c32(&colors.sidebar_bg)))
                .show(ui, |ui| {
                    let r = ui.max_rect();
                    ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, c32(&colors.separator)));
                    let data = sidebar::Data {
                        listing: self.listing.as_ref(),
                        loading: self.is_loading_listing(),
                        schemas: &self.schemas,
                        schema: &self.schema,
                        favorites: &self.favorites,
                        history: &self.history,
                        active: active_item.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
                        colors: &colors,
                    };
                    sidebar::show(ui, &mut self.sidebar, &data, &mut actions);
                });
        }

        // tab strip + content
        egui::CentralPanel::default().frame(egui::Frame::new().fill(c32(&colors.content_bg))).show(ui, |ui| {
            egui::Panel::top("tab_strip").exact_size(30.0).frame(egui::Frame::new().fill(c32(&colors.content_bg))).show(ui, |ui| {
                self.tab_strip(ui, env);
            });
            if self.tabs.is_empty() {
                let area = ui.available_rect_before_wrap();
                let w = (area.width() * 0.3).clamp(160.0, 300.0);
                widgets::paint_elephant(ui.painter(), area.center() - egui::vec2(0.0, 10.0), w, &colors);
                ui.painter().text(
                    egui::pos2(area.center().x, area.center().y + w * 0.72 / 2.0 + 24.0),
                    egui::Align2::CENTER_CENTER,
                    "Pick a table in the sidebar · Cmd+T for a SQL tab · Cmd+K to switch database",
                    egui::FontId::proportional(12.0),
                    c32(&colors.text_dim),
                );
                return;
            }
            let session = self.session.clone();
            let active = self.active.min(self.tabs.len() - 1);
            let Workspace { tabs, completion, .. } = self;
            for (i, tab) in tabs.iter_mut().enumerate() {
                let mut cx = TabCx { env: &mut *env, session: &session, actions: &mut actions, active: i == active, modal_open: blocked };
                if i != active {
                    match &mut tab.kind {
                        Kind::Table(t) => t.poll(&mut cx),
                        Kind::Query(q) => q.poll(&mut cx),
                        _ => {}
                    }
                    continue;
                }
                match &mut tab.kind {
                    Kind::Table(t) => t.ui(ui, &mut cx),
                    Kind::Query(q) => q.ui(ui, &mut cx, completion),
                    Kind::Processes(p) => p.ui(ui, &mut cx),
                    Kind::Definition(d) => {
                        if let Some(r) = poll_slot(&mut d.task) {
                            d.text = Some(r.map(|t| t).unwrap_or_else(|e| format!("-- {e}")));
                        }
                        match &d.text {
                            Some(text) => {
                                ui.add_space(8.0);
                                egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
                                    if widgets::small_button(ui, "Copy").clicked() {
                                        ui.ctx().copy_text(text.clone());
                                    }
                                    ui.add_space(4.0);
                                    editor::sql_view(ui, &colors, text, ui.available_height() - 12.0);
                                });
                            }
                            None => {
                                ui.add_space(40.0);
                                ui.vertical_centered(|ui| ui.spinner());
                            }
                        }
                    }
                }
            }
        });

        self.dialogs(&ctx, env, &mut actions);
        self.process_actions(env, actions);
        if let Some(db) = self.open_db_event.take() {
            event = Some(Event::OpenDatabase(db));
        }
        event
    }

    fn reload_active(&mut self, env: &mut Env, actions: &mut Vec<Action>) {
        let session = self.session.clone();
        let active = self.active;
        if let Some(Tab { kind: Kind::Table(t), .. }) = self.tabs.get_mut(active) {
            let mut cx = TabCx { env, session: &session, actions, active: true, modal_open: false };
            t.reload(&mut cx);
        }
        actions.push(Action::RefreshListing);
    }

    fn begin_reconnect(&mut self, env: &mut Env) {
        let s = self.session.clone();
        self.reconnect = Some(env.sp.spawn(async move { s.reconnect().await.map_err(err_msg) }));
    }

    fn open_processes(&mut self, env: &mut Env) {
        if let Some(i) = self.tabs.iter().position(|t| matches!(t.kind, Kind::Processes(_))) {
            self.active = i;
            return;
        }
        let session = self.session.clone();
        let mut sub = vec![];
        let p = {
            let mut cx = TabCx { env, session: &session, actions: &mut sub, active: true, modal_open: false };
            ProcessTab::new(&mut cx)
        };
        self.pending.extend(sub);
        self.add_tab(Kind::Processes(p), true);
    }

    fn open_db_dialog(&mut self, env: &mut Env) {
        let s = self.session.clone();
        let task = env.sp.spawn(async move { s.list_databases().await.map_err(err_msg) });
        self.dialog = Some(Dialog::OpenDb { dbs: None, task: Some(task) });
    }

    fn tab_strip(&mut self, ui: &mut Ui, env: &Env) {
        let colors = env.colors.clone();
        let mut close = None;
        let mut select = None;
        let mut pin = None;
        let mut new_query = false;
        let strip = ui.max_rect();
        ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, c32(&colors.control_border)));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            egui::ScrollArea::horizontal().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (i, t) in self.tabs.iter().enumerate() {
                        let mut title = t.title();
                        if title.chars().count() > 26 {
                            title = title.chars().take(25).collect::<String>() + "…";
                        }
                        let font = egui::FontId::proportional(13.0);
                        let selected = i == self.active;
                        let probe = ui.painter().layout_no_wrap(title.clone(), font.clone(), egui::Color32::WHITE);
                        let width = (probe.size().x + 56.0).clamp(110.0, 220.0);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, ui.available_height()), Sense::click());
                        let fill = if selected { c32(&colors.tab_selected) } else if resp.hovered() { c32(&colors.tab_hover) } else { c32(&colors.tab_bg) };
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
                        ui.painter().line_segment([rect.right_top(), rect.right_bottom()], Stroke::new(1.0, c32(&colors.separator)));
                        let text_col = if selected { egui::Color32::WHITE } else { c32(&colors.tab_text) };
                        let icon_c = egui::pos2(rect.left() + 17.0, rect.center().y);
                        if t.busy() {
                            ui.painter().circle_filled(icon_c, 3.5, c32(&colors.key_color));
                        } else {
                            crate::widgets::paint_tab_icon(ui.painter(), icon_c, t.icon_kind(), text_col.gamma_multiply(0.75));
                        }
                        let galley = ui.painter().layout_no_wrap(title, font, text_col);
                        let label_pos = egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0);
                        ui.painter().with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - 24.0, rect.bottom()))).galley(label_pos, galley, text_col);
                        if !t.pinned {
                            // preview tabs (replaced by the next single click) get an underline
                            let y = rect.center().y + 8.0;
                            ui.painter().line_segment([egui::pos2(label_pos.x, y), egui::pos2(rect.right() - 28.0, y)], Stroke::new(1.0, text_col.gamma_multiply(0.45)));
                        }
                        let x_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - 16.0, rect.center().y), egui::vec2(16.0, 16.0));
                        let x_resp = ui.interact(x_rect, ui.id().with(("tabx", t.id)), Sense::click());
                        if resp.hovered() || selected || x_resp.hovered() {
                            if x_resp.hovered() {
                                ui.painter().rect_filled(x_rect, CornerRadius::same(3), egui::Color32::from_white_alpha(48));
                            }
                            let c = x_rect.center();
                            let st = Stroke::new(1.3, text_col);
                            ui.painter().line_segment([c - egui::vec2(3.0, 3.0), c + egui::vec2(3.0, 3.0)], st);
                            ui.painter().line_segment([c - egui::vec2(3.0, -3.0), c + egui::vec2(3.0, -3.0)], st);
                        }
                        if x_resp.clicked() || resp.middle_clicked() {
                            close = Some(i);
                        } else if resp.clicked() {
                            select = Some(i);
                        }
                        if resp.double_clicked() {
                            pin = Some(i);
                        }
                    }
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(32.0, ui.available_height()), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::ZERO, c32(&colors.tab_hover));
                    }
                    let c = rect.center();
                    let st = Stroke::new(1.4, c32(&colors.text_dim));
                    ui.painter().line_segment([c - egui::vec2(5.0, 0.0), c + egui::vec2(5.0, 0.0)], st);
                    ui.painter().line_segment([c - egui::vec2(0.0, 5.0), c + egui::vec2(0.0, 5.0)], st);
                    if resp.on_hover_text("New SQL query (Cmd+T)").clicked() {
                        new_query = true;
                    }
                });
            });
        });
        if new_query {
            self.pending.push(Action::OpenQuery { title: "SQL".into(), sql: String::new() });
        }
        if let Some(i) = select {
            self.active = i;
        }
        if let Some(i) = pin {
            self.tabs[i].pinned = true;
        }
        if let Some(i) = close {
            self.close_tab(i);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, env: &mut Env, _actions: &mut Vec<Action>) {
        let colors = env.colors.clone();
        let Some(dialog) = &mut self.dialog else { return };
        let mut close = false;
        let mut run_sql: Option<(String, Option<(String, String)>)> = None;
        let mut open_db: Option<String> = None;
        let mut create: Option<(Vec<DdlOp>, String)> = None;

        match dialog {
            Dialog::Confirm(d) => {
                let m = widgets::modal(ctx, &colors, "confirm").show(ctx, |ui| {
                    ui.set_width(420.0);
                    ui.heading(&d.title);
                    ui.add_space(6.0);
                    ui.label(&d.message);
                    ui.add_space(4.0);
                    ui.label(RichText::new(&d.sql).monospace().color(c32(&colors.text_dim)));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let fill = if d.danger { c32(&colors.status_err) } else { c32(&colors.accent) };
                        if ui.add(egui::Button::new(RichText::new(&d.confirm).color(egui::Color32::WHITE)).fill(fill).min_size(egui::vec2(72.0, 26.0))).clicked() {
                            run_sql = Some((d.sql.clone(), d.close_table.clone()));
                        }
                        if widgets::button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
                if m.should_close() {
                    close = true;
                }
                if run_sql.is_some() {
                    close = true;
                }
            }
            Dialog::Rename { schema, name, kind, text } => {
                let m = widgets::modal(ctx, &colors, "rename").show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading(format!("Rename {name}"));
                    ui.add_space(8.0);
                    let r = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                    r.request_focus();
                    let valid = !text.trim().is_empty() && text.trim() != name.as_str();
                    ui.add_space(10.0);
                    let mut go = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && valid;
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, &colors, "Rename", valid).clicked() {
                            go = true;
                        }
                        if widgets::button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                    if go {
                        let kw = match kind.as_str() {
                            "view" => "VIEW",
                            "matview" => "MATERIALIZED VIEW",
                            "foreign" => "FOREIGN TABLE",
                            _ => "TABLE",
                        };
                        run_sql = Some((format!("ALTER {kw} {} RENAME TO {}", qualified(schema, name), quote_ident(text.trim())), Some((schema.clone(), name.clone()))));
                    }
                });
                if m.should_close() {
                    close = true;
                }
                if run_sql.is_some() {
                    close = true;
                }
            }
            Dialog::OpenDb { dbs, task } => {
                if let Some(t) = task {
                    if let Some(r) = t.poll() {
                        *task = None;
                        match r {
                            Ok(v) => *dbs = Some(v),
                            Err(e) => {
                                env.err(e.to_string());
                                close = true;
                            }
                        }
                    }
                }
                let current = self.session.info.database.clone();
                let m = widgets::modal(ctx, &colors, "open_db").show(ctx, |ui| {
                    ui.set_width(340.0);
                    ui.heading("Open database");
                    ui.add_space(8.0);
                    match dbs {
                        None => {
                            ui.spinner();
                        }
                        Some(v) => {
                            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                for db in v.iter() {
                                    let label = if *db == current { format!("{db}  (current)") } else { db.clone() };
                                    if ui.selectable_label(*db == current, label).clicked() && *db != current {
                                        open_db = Some(db.clone());
                                    }
                                }
                            });
                        }
                    }
                    ui.add_space(10.0);
                    if widgets::button(ui, "Cancel").clicked() {
                        close = true;
                    }
                });
                if m.should_close() {
                    close = true;
                }
            }
            Dialog::CreateTable(ct) => {
                let schema = self.schema.clone();
                let m = widgets::modal(ctx, &colors, "create_table").show(ctx, |ui| {
                    ui.set_width(640.0);
                    ui.heading(format!("Create table in {schema}"));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut ct.name).desired_width(240.0));
                    });
                    ui.add_space(6.0);
                    egui::Grid::new("ct_cols").num_columns(5).spacing([10.0, 6.0]).show(ui, |ui| {
                        for h in ["Column", "Type", "Null", "PK", ""] {
                            ui.label(RichText::new(h).size(11.0).color(c32(&colors.text_dim)));
                        }
                        ui.end_row();
                        let mut remove = None;
                        for (i, (n, t, nullable, pk)) in ct.cols.iter_mut().enumerate() {
                            ui.add(egui::TextEdit::singleline(n).desired_width(170.0));
                            structure::type_picker(ui, t, 150.0);
                            ui.checkbox(nullable, "");
                            ui.checkbox(pk, "");
                            if widgets::small_button(ui, "×").clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                        if let Some(i) = remove {
                            ct.cols.remove(i);
                        }
                    });
                    if widgets::small_button(ui, "+ Column").clicked() {
                        ct.cols.push((String::new(), "text".into(), true, false));
                    }
                    let ops = vec![DdlOp::CreateTable {
                        name: ct.name.trim().to_string(),
                        columns: ct
                            .cols
                            .iter()
                            .filter(|c| !c.0.trim().is_empty())
                            .map(|(n, t, nl, pk)| NewColumn { name: n.trim().into(), data_type: t.trim().into(), nullable: *nl && !*pk, default: None, primary_key: *pk })
                            .collect(),
                    }];
                    let sql = ddl_sql(&schema, "", &ops).join(";\n");
                    ui.add_space(8.0);
                    if !ct.name.trim().is_empty() {
                        editor::sql_view(ui, &colors, &format!("{sql};"), 150.0);
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, &colors, "Create", !ct.name.trim().is_empty() && !ct.cols.is_empty()).clicked() {
                            create = Some((ops, ct.name.trim().to_string()));
                        }
                        if widgets::button(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
                if m.should_close() {
                    close = true;
                }
            }
        }

        if let Some((sql, close_table)) = run_sql {
            self.start_exec(env, sql, close_table);
        }
        if let Some((ops, name)) = create {
            let s = self.session.clone();
            let schema = self.schema.clone();
            self.exec = Some((env.sp.spawn(async move { s.apply_ddl(&schema, "", &ops).await.map(|v| v.len() as u64).map_err(err_msg) }), None));
            self.open_after_exec = Some(name);
            close = true;
        }
        if let Some(db) = open_db {
            self.dialog = None;
            self.open_db_event = Some(db);
            return;
        }
        if close {
            self.dialog = None;
        }
    }
}

#[allow(dead_code)]
fn _unused(_: Align, _: Layout, _: StrokeKind) {}
