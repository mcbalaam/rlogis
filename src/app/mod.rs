use std::collections::HashSet;
use std::ops::Range;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use crate::log::{Category, LogFile, LogRow, Mark, MarkKind, PlayerKey};
use crate::platform::{self, PickOutcome};
use crate::theme::{Theme, ThemeKind};

mod badges;
mod filters;
mod lists;
mod search;
mod table;
mod toolbar;

use table::table_ui;

const THEME_KEY: &str = "rlogis.theme";
const ACCENT_KEY: &str = "rlogis.row_accent";
const DEDUP_KEY: &str = "rlogis.dedup";
const FORMAT_KEY: &str = "rlogis.format";
const SENSITIVE_KEY: &str = "rlogis.hide_sensitive";
const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 27.0;

const OPEN_LOG: &str = "Open";
const OPENING_FILE: &str = "Opening file…";
const OPEN_FAILED: &str = "Could not open the file";

pub struct RlogisApp {
    theme: ThemeKind,
    applied_theme: Option<ThemeKind>,
    row_accent: bool,
    dedup: bool,
    format: bool,
    hide_sensitive: bool,
    log: Option<LogFile>,
    picker: Option<Receiver<PickOutcome>>,
    error: Option<String>,
    copied: Option<CopiedText>,
    search: SearchState,
    lists: ListState,
    filters: FilterState,
}

struct CopiedText {
    text: String,
    until: f64,
}

#[derive(Default)]
struct SearchState {
    open: bool,
    focus: bool,
    query: String,
    active_query: String,
    matches: Vec<usize>,
    current: usize,
    scroll_to: Option<usize>,
    highlight: Option<(usize, f64)>,
}

impl RlogisApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|value| ThemeKind::from_code(&value))
            .unwrap_or_default();
        let row_accent = cc
            .storage
            .and_then(|storage| storage.get_string(ACCENT_KEY))
            .is_some_and(|value| value == "1");
        let dedup = cc
            .storage
            .and_then(|storage| storage.get_string(DEDUP_KEY))
            .is_some_and(|value| value == "1");
        let format = cc
            .storage
            .and_then(|storage| storage.get_string(FORMAT_KEY))
            .is_some_and(|value| value == "1");
        let hide_sensitive = cc
            .storage
            .and_then(|storage| storage.get_string(SENSITIVE_KEY))
            .is_some_and(|value| value == "1");

        theme.theme().apply(&cc.egui_ctx);

        Self {
            theme,
            applied_theme: Some(theme),
            row_accent,
            dedup,
            format,
            hide_sensitive,
            log: None,
            picker: None,
            error: None,
            copied: None,
            search: SearchState::default(),
            lists: ListState::default(),
            filters: FilterState::default(),
        }
    }

    fn open_picker(&mut self, ctx: &egui::Context) {
        if self.picker.is_some() {
            return;
        }

        let (tx, rx) = mpsc::channel();
        platform::pick_log_file(ctx, tx);
        self.picker = Some(rx);
        self.error = None;
    }

    fn poll_picker(&mut self) {
        let Some(rx) = &self.picker else {
            return;
        };

        match rx.try_recv() {
            Ok(outcome) => {
                self.picker = None;
                match outcome {
                    PickOutcome::Loaded { bytes } => {
                        let text = String::from_utf8_lossy(&bytes);
                        self.log = Some(LogFile::parse(&text));
                        self.filters.dirty = true;
                        self.error = None;
                    }
                    PickOutcome::Cancelled => {}
                    PickOutcome::Failed(message) => self.error = Some(message),
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.picker = None;
                self.error = Some("file picker closed unexpectedly".to_owned());
            }
        }
    }

    fn error_banner(&mut self, ui: &mut Ui) {
        let Some(error) = self.error.clone() else {
            return;
        };
        let theme = self.theme.theme();

        egui::Panel::top("error_banner")
            .frame(
                egui::Frame::new()
                    .fill(theme.top_bar)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{OPEN_FAILED}: {error}")).color(theme.danger));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").on_hover_text("Close").clicked() {
                            self.error = None;
                        }
                    });
                });
            });
    }

    fn central(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            let scroll_to = self.search.scroll_to.take();
            let now = ui.ctx().input(|input| input.time);
            let highlight = self.search.highlight.map(|(row, started)| {
                let remaining = (1.0 - (now - started)).clamp(0.0, 1.0) as f32;
                (row, remaining)
            });

            let Some(log) = self.log.as_ref() else {
                self.empty_state(ui);
                return;
            };

            if self.filters.visible.is_empty() {
                let top_gap = (ui.available_height() * 0.4).max(20.0);
                ui.add_space(top_gap);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No entries match the filters").weak());
                });
                return;
            }

            let indices = self.filters.visible.as_slice();
            table_ui(
                ui,
                log,
                indices,
                self.theme,
                ViewOptions {
                    accent: self.row_accent,
                    dedup: self.dedup,
                    format: self.format,
                    hide_sensitive: self.hide_sensitive,
                    highlight,
                },
                scroll_to,
                &mut self.copied,
            );
        });
    }

    fn empty_state(&mut self, ui: &mut Ui) {
        let theme = self.theme.theme();

        let top_gap = ((ui.available_height() - 80.0) * 0.5).max(20.0);
        ui.add_space(top_gap);
        ui.vertical_centered(|ui| {
            let button =
                egui::Button::new(RichText::new(OPEN_LOG).size(19.0).color(theme.accent_text))
                    .fill(theme.accent)
                    .corner_radius(CornerRadius::same(10))
                    .min_size(egui::vec2(250.0, 54.0));

            if ui.add(button).clicked() {
                self.open_picker(ui.ctx());
            }
        });
    }
}

impl eframe::App for RlogisApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_picker();

        let ctx = ui.ctx().clone();
        if self.applied_theme != Some(self.theme) {
            self.theme.theme().apply(&ctx);
            self.applied_theme = Some(self.theme);
        }

        if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::F)) {
            self.search.open = true;
            self.search.focus = true;
        }

        let now = ctx.input(|input| input.time);
        if self
            .copied
            .as_ref()
            .is_some_and(|copied| now >= copied.until)
        {
            self.copied = None;
        }

        if let Some((_, started)) = self.search.highlight {
            if now - started >= 1.0 {
                self.search.highlight = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }

        self.refresh_visible_rows();

        self.toolbar(ui);
        self.error_banner(ui);
        self.central(ui);
        self.search_window(&ctx);
        self.list_windows(&ctx);
        self.filter_window(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.code().to_owned());
        storage.set_string(
            ACCENT_KEY,
            if self.row_accent { "1" } else { "0" }.to_owned(),
        );
        storage.set_string(DEDUP_KEY, if self.dedup { "1" } else { "0" }.to_owned());
        storage.set_string(FORMAT_KEY, if self.format { "1" } else { "0" }.to_owned());
        storage.set_string(
            SENSITIVE_KEY,
            if self.hide_sensitive { "1" } else { "0" }.to_owned(),
        );
    }
}

#[derive(Clone, Copy)]
struct ViewOptions {
    accent: bool,
    dedup: bool,
    format: bool,
    hide_sensitive: bool,
    highlight: Option<(usize, f32)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum IdentityMode {
    /// Show rows that contain any selected ckey/name
    #[default]
    Add,
    /// Show only rows that contain every selected ckey/name
    Overlap,
}

struct FilterState {
    open: bool,
    enabled: [bool; Category::COUNT],
    /// Original row indices after category filters (and Dedup)
    visible: Vec<u32>,
    dirty: bool,
    type_search: String,
    /// Identity filters: rows must contain at least one selected ckey/name
    selected_keys: HashSet<u32>,
    selected_names: HashSet<String>,
    identity_mode: IdentityMode,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            open: false,
            enabled: [true; Category::COUNT],
            visible: Vec::new(),
            dirty: true,
            type_search: String::new(),
            selected_keys: HashSet::new(),
            selected_names: HashSet::new(),
            identity_mode: IdentityMode::Add,
        }
    }
}

#[derive(Default)]
struct ListState {
    ckeys: bool,
    names: bool,
    ckeys_search: String,
    names_search: String,
}
