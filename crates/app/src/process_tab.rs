//! Process list (pg_stat_activity) with cancel / terminate.

use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Layout, RichText, Ui};
use table_plus_core::model::ProcessInfo;

use crate::app::err_msg;
use crate::task::{poll_slot, Task};
use crate::theme::c32;
use crate::widgets;
use crate::workspace::TabCx;

pub struct ProcessTab {
    rows: Vec<ProcessInfo>,
    task: Option<Task<Vec<ProcessInfo>>>,
    kill: Option<Task<bool>>,
    auto: bool,
    last: Option<Instant>,
}

impl ProcessTab {
    pub fn new(cx: &mut TabCx) -> Self {
        let mut t = Self { rows: vec![], task: None, kill: None, auto: false, last: None };
        t.refresh(cx);
        t
    }

    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    fn refresh(&mut self, cx: &mut TabCx) {
        if self.task.is_some() {
            return;
        }
        let s = cx.session.clone();
        self.last = Some(Instant::now());
        self.task = Some(cx.env.sp.spawn(async move { s.processes().await.map_err(err_msg) }));
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut TabCx) {
        if let Some(r) = poll_slot(&mut self.task) {
            match r {
                Ok(rows) => self.rows = rows,
                Err(e) => cx.env.err(format!("Process list: {e}")),
            }
        }
        if let Some(r) = poll_slot(&mut self.kill) {
            match r {
                Ok(true) => {
                    cx.env.ok("Signal sent");
                    self.refresh(cx);
                }
                Ok(false) => cx.env.err("The server refused (not your backend?)"),
                Err(e) => cx.env.err(e.to_string()),
            }
        }
        if self.auto {
            if self.last.map(|t| t.elapsed() > Duration::from_secs(3)).unwrap_or(true) {
                self.refresh(cx);
            }
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        let colors = cx.env.colors.clone();
        egui::Frame::new().fill(c32(&colors.sidebar_bg)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("Process list").strong());
                if widgets::small_button(ui, "Refresh").clicked() {
                    self.refresh(cx);
                }
                ui.checkbox(&mut self.auto, "Auto-refresh");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(widgets::dim(ui, format!("{} backend(s)", self.rows.len())));
                });
            });
        });
        let mut signal: Option<(i32, bool)> = None;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("process_grid").num_columns(9).striped(true).spacing([14.0, 6.0]).min_row_height(24.0).show(ui, |ui| {
                for h in ["PID", "User", "Database", "App", "State", "Wait", "Started", "Query", ""] {
                    ui.label(RichText::new(h).size(11.0).color(c32(&colors.text_dim)));
                }
                ui.end_row();
                for p in &self.rows {
                    ui.label(p.pid.to_string());
                    ui.label(p.user.clone().unwrap_or_default());
                    ui.label(p.database.clone().unwrap_or_default());
                    ui.label(p.application.clone().unwrap_or_default());
                    ui.label(p.state.clone().unwrap_or_default());
                    ui.label(p.wait_event.clone().unwrap_or_default());
                    ui.label(p.query_start.clone().unwrap_or_default());
                    let q = p.query.clone().unwrap_or_default().replace('\n', " ");
                    let short: String = q.chars().take(90).collect();
                    ui.label(RichText::new(short).monospace()).on_hover_text(q);
                    ui.horizontal(|ui| {
                        if widgets::small_button(ui, "Cancel").clicked() {
                            signal = Some((p.pid, false));
                        }
                        if widgets::small_button(ui, "Terminate").clicked() {
                            signal = Some((p.pid, true));
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if let Some((pid, terminate)) = signal {
            let s = cx.session.clone();
            self.kill = Some(cx.env.sp.spawn(async move { s.kill_process(pid, terminate).await.map_err(err_msg) }));
        }
    }
}
