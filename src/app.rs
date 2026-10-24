//! Application state and UI.

use std::ops::Range;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{Align, Color32, CornerRadius, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use crate::log::{Category, LogFile, LogRow, Mark, MarkKind, PlayerKey};
use crate::platform::{self, PickOutcome};
use crate::theme::{Theme, ThemeKind};

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
    /// Row to flash and when the 1-second fade started.
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

    fn toolbar(&mut self, ui: &mut Ui) {
        let theme = self.theme.theme();

        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(theme.top_bar)
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let picking = self.picker.is_some();
                    if ui
                        .add_enabled(!picking, egui::Button::new(OPEN_LOG))
                        .clicked()
                    {
                        self.open_picker(ui.ctx());
                    }

                    if picking {
                        ui.spinner();
                        ui.label(RichText::new(OPENING_FILE).weak());
                    }

                    if ui
                        .button("Find")
                        .on_hover_text("Exact search (Ctrl+F)")
                        .clicked()
                    {
                        self.search.open = true;
                        self.search.focus = true;
                    }

                    ui.toggle_value(&mut self.lists.ckeys, "Ckeys")
                        .on_hover_text("All player keys");
                    ui.toggle_value(&mut self.lists.names, "Names")
                        .on_hover_text("All character names");
                    ui.toggle_value(&mut self.lists.pairs, "Pairs")
                        .on_hover_text("Character name with its player key");
                    ui.toggle_value(&mut self.filters.open, "Filters")
                        .on_hover_text("Hide categories");

                    ui.separator();

                    ui.label(RichText::new("Theme").weak());
                    egui::ComboBox::from_id_salt("theme_combo")
                        .selected_text(self.theme.label())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            for theme in ThemeKind::ALL {
                                ui.selectable_value(&mut self.theme, theme, theme.label());
                            }
                        });

                    ui.checkbox(&mut self.row_accent, "Accent")
                        .on_hover_text("Tint every row with the accent color of its log type");
                    if ui
                        .checkbox(&mut self.dedup, "Dedup")
                        .on_hover_text(
                            "Collapse duplicate badges and hide repeated tgui-window lines from one player",
                        )
                        .changed()
                    {
                        self.filters.dirty = true;
                        self.search.active_query.clear();
                        self.search.matches.clear();
                        self.search.scroll_to = None;
                    }
                    ui.checkbox(&mut self.format, "Format").on_hover_text(
                        "Clean log syntax and HTML: ckey, character name, then the message",
                    );
                    ui.checkbox(&mut self.hide_sensitive, "Hide sensitive")
                        .on_hover_text("Hide keys, IP addresses and session IDs");

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(log) = &self.log {
                            let entries = entries_label(log.entry_count);
                            let rows = rows_label(self.filters.visible.len());
                            let response =
                                ui.label(RichText::new(format!("{entries} · {rows}")).weak());
                            if let Some((first, last)) = log.time_range() {
                                response.on_hover_text(format!(
                                    "{} {} – {}",
                                    first.date_string(),
                                    first,
                                    last
                                ));
                            }
                        }
                    });
                });
            });
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

    /// The initial screen is just the centered action button.
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

    fn search_window(&mut self, ctx: &egui::Context) {
        if !self.search.open {
            return;
        }

        let mut open = true;
        let mut submit = false;
        let mut step = 0_isize;
        let content = ctx.content_rect();
        let default_pos = egui::pos2(content.center().x - 235.0, content.top() + 64.0);

        egui::Window::new("Find")
            .open(&mut open)
            .default_pos(default_pos)
            .movable(true)
            .constrain(false)
            .collapsible(false)
            .resizable(false)
            .default_width(470.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add_sized(
                        [330.0, 22.0],
                        egui::TextEdit::singleline(&mut self.search.query),
                    );

                    if self.search.focus {
                        response.request_focus();
                        self.search.focus = false;
                    }

                    if response.changed() {
                        self.search.active_query.clear();
                        self.search.matches.clear();
                        self.search.scroll_to = None;
                    }

                    let enter = ui.input(|input| input.key_pressed(egui::Key::Enter))
                        && (response.has_focus() || response.lost_focus());
                    if enter || ui.button("Find").clicked() {
                        submit = true;
                    }
                });

                if !self.search.active_query.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} / {}",
                            self.search.current + 1,
                            self.search.matches.len()
                        ));
                        if ui.small_button("◀").clicked() && !self.search.matches.is_empty() {
                            step = -1;
                        }
                        if ui.small_button("▶").clicked() && !self.search.matches.is_empty() {
                            step = 1;
                        }
                    });
                }
            });

        self.search.open = open;
        if submit {
            self.submit_search();
            self.search.focus = true;
        }
        if step != 0 {
            self.step_search(step);
        }
        if (submit || step != 0)
            && let Some(&row) = self.search.matches.get(self.search.current)
        {
            self.search.highlight = Some((row, ctx.input(|input| input.time)));
            ctx.request_repaint();
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.search.open = false;
        }
    }

    /// Ckeys, names and pairs windows. Each one is a separate movable window.
    fn list_windows(&mut self, ctx: &egui::Context) {
        if !(self.lists.ckeys || self.lists.names || self.lists.pairs) {
            return;
        }

        let theme_kind = self.theme;
        let hide_sensitive = self.hide_sensitive;
        let content = ctx.content_rect();
        let Self {
            log, lists, copied, ..
        } = self;
        let Some(log) = log.as_ref() else {
            return;
        };
        let theme = theme_kind.theme();

        let mut ckeys: Vec<usize> = (0..log.keys.len())
            .filter(|&index| !log.keys[index].system)
            .collect();
        ckeys.sort_by(|&a, &b| log.keys[a].ckey.cmp(&log.keys[b].ckey));

        let mut names: Vec<(&str, u32)> = log
            .characters
            .iter()
            .map(|character| (character.name.as_str(), character.key))
            .collect();
        names.sort();
        names.dedup_by(|a, b| a.0 == b.0);

        let base = egui::pos2(content.left() + 60.0, content.top() + 90.0);

        if lists.ckeys {
            let mut open = true;
            egui::Window::new("Ckeys")
                .open(&mut open)
                .default_pos(base)
                .default_width(360.0)
                .max_width(380.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(340.0);
                    ui.label(RichText::new(format!("{} keys", ckeys.len())).weak());
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for &index in &ckeys {
                                    let player = &log.keys[index];
                                    key_chip(ui, player, index, theme, hide_sensitive, copied);
                                }
                            });
                        });
                });
            lists.ckeys = open;
        }

        if lists.names {
            let mut open = true;
            egui::Window::new("Names")
                .open(&mut open)
                .default_pos(base + egui::vec2(30.0, 30.0))
                .default_width(360.0)
                .max_width(380.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(340.0);
                    ui.label(RichText::new(format!("{} names", names.len())).weak());
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for &(name, key) in &names {
                                    let (fill, text) = player_colors(theme, log, key);
                                    copyable_badge(
                                        ui,
                                        name,
                                        (fill, text),
                                        true,
                                        copied,
                                        &format!("Character name: {name}\nClick to copy"),
                                    );
                                }
                            });
                        });
                });
            lists.names = open;
        }

        if lists.pairs {
            let mut open = true;
            let mut pairs: Vec<(&str, u32)> = log
                .characters
                .iter()
                .map(|character| (character.name.as_str(), character.key))
                .collect();
            pairs.sort_by(|a, b| {
                log.keys[a.1 as usize]
                    .ckey
                    .cmp(&log.keys[b.1 as usize].ckey)
                    .then_with(|| a.0.cmp(b.0))
            });
            egui::Window::new("Pairs")
                .open(&mut open)
                .default_pos(base + egui::vec2(60.0, 60.0))
                .default_width(420.0)
                .max_width(440.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(400.0);
                    ui.label(RichText::new(format!("{} pairs", pairs.len())).weak());
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for &(name, key) in &pairs {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    let (fill, text) = player_colors(theme, log, key);
                                    copyable_badge(
                                        ui,
                                        name,
                                        (fill, text),
                                        true,
                                        copied,
                                        &format!("Character name: {name}\nClick to copy"),
                                    );
                                    key_chip(
                                        ui,
                                        &log.keys[key as usize],
                                        key as usize,
                                        theme,
                                        hide_sensitive,
                                        copied,
                                    );
                                });
                            }
                        });
                });
            lists.pairs = open;
        }
    }

    /// Recomputes the visible row list after a filter or Dedup change.
    fn refresh_visible_rows(&mut self) {
        if !self.filters.dirty {
            return;
        }

        let enabled = self.filters.enabled;
        let dedup = self.dedup;
        let mut visible = Vec::new();

        if let Some(log) = &self.log {
            if dedup {
                for &index in &log.dedup_indices {
                    if enabled[log.rows[index as usize].category.index()] {
                        visible.push(index);
                    }
                }
            } else {
                for (index, row) in log.rows.iter().enumerate() {
                    if enabled[row.category.index()] {
                        visible.push(index as u32);
                    }
                }
            }
        }

        self.filters.visible = visible;
        self.filters.dirty = false;
    }

    fn filter_window(&mut self, ctx: &egui::Context) {
        if !self.filters.open {
            return;
        }

        let theme = self.theme.theme();
        let mut open = true;
        let mut changed = false;
        let content = ctx.content_rect();
        let position = egui::pos2(content.left() + 80.0, content.top() + 100.0);

        egui::Window::new("Filters")
            .open(&mut open)
            .default_pos(position)
            .movable(true)
            .collapsible(false)
            .resizable(false)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("All").clicked() {
                        self.filters.enabled = [true; Category::COUNT];
                        changed = true;
                    }
                    if ui.small_button("None").clicked() {
                        self.filters.enabled = [false; Category::COUNT];
                        changed = true;
                    }
                });
                ui.separator();

                for category in Category::ALL {
                    let index = category.index();
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut self.filters.enabled[index], "").changed() {
                            changed = true;
                        }
                        ui.label(
                            RichText::new(category.as_str())
                                .monospace()
                                .color(theme.category_color(category)),
                        );
                    });
                }
            });

        self.filters.open = open;
        if changed {
            self.filters.dirty = true;
            self.search.active_query.clear();
            self.search.matches.clear();
            self.search.scroll_to = None;
        }
    }

    fn submit_search(&mut self) {
        let query = self.search.query.trim().to_owned();
        let same_query = query == self.search.active_query;
        self.search.active_query = query.clone();

        if query.is_empty() {
            self.search.matches.clear();
            self.search.current = 0;
            self.search.scroll_to = None;
            return;
        }

        if same_query && !self.search.matches.is_empty() {
            self.search.scroll_to = self.search.matches.get(self.search.current).copied();
            return;
        }

        let Some(log) = &self.log else {
            self.search.matches.clear();
            return;
        };

        let mut matches: Vec<usize> = Vec::new();
        let query_lower = query.to_lowercase();
        for (display_index, &original_index) in self.filters.visible.iter().enumerate() {
            if row_matches(&log.rows[original_index as usize], &query_lower, &log.keys) {
                matches.push(display_index);
            }
        }

        if matches.is_empty() {
            self.search.matches.clear();
            self.search.current = 0;
            self.search.scroll_to = None;
            return;
        }

        self.search.current = 0;
        self.search.scroll_to = matches.first().copied();
        self.search.matches = matches;
    }

    fn step_search(&mut self, step: isize) {
        if self.search.matches.is_empty() {
            return;
        }
        let len = self.search.matches.len() as isize;
        self.search.current = (self.search.current as isize + step).rem_euclid(len) as usize;
        self.search.scroll_to = self.search.matches.get(self.search.current).copied();
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

fn row_matches(row: &LogRow, query_lower: &str, keys: &[PlayerKey]) -> bool {
    let contains = |text: &str| text.to_lowercase().contains(query_lower);

    contains(&row.message)
        || row
            .formatted
            .as_ref()
            .is_some_and(|formatted| contains(&formatted.text))
        || contains(row.category.as_str())
        || row.badges.iter().any(|badge| {
            keys.get(badge.key as usize)
                .is_some_and(|player| contains(&player.key) || contains(&player.ckey))
        })
}

fn table_ui(
    ui: &mut Ui,
    log: &LogFile,
    indices: &[u32],
    theme_kind: ThemeKind,
    view: ViewOptions,
    scroll_to: Option<usize>,
    copied: &mut Option<CopiedText>,
) {
    let theme = theme_kind.theme();

    let mut table = TableBuilder::new(ui)
        .id_salt("log_table")
        .striped(!view.accent)
        .resizable(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(86.0))
        .column(Column::exact(112.0))
        .column(Column::exact(124.0))
        .column(Column::remainder().clip(true))
        .min_scrolled_height(0.0)
        .auto_shrink([false, false]);

    if let Some(row) = scroll_to {
        table = table.scroll_to_row(row, Some(Align::Center));
    }

    table
        .header(HEADER_HEIGHT, |mut header| {
            for label in ["#", "Time", "Type", "Message"] {
                header.col(|ui| {
                    paint_cell(ui, theme.header);
                    ui.label(RichText::new(label).strong().color(theme.text));
                });
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, indices.len(), |mut row| {
                let display_index = row.index();
                let original_index = indices[display_index] as usize;
                let entry = &log.rows[original_index];
                render_row(&mut row, entry, log, theme, view, copied);
            });
        });
}

fn render_row(
    row: &mut TableRow<'_, '_>,
    entry: &LogRow,
    log: &LogFile,
    theme: &Theme,
    view: ViewOptions,
    copied: &mut Option<CopiedText>,
) {
    let row_index = row.index();
    let accent_bg = view
        .accent
        .then(|| theme.row_accent(entry.category, entry.continuation));
    let highlight_bg = view.highlight.and_then(|(highlight_row, strength)| {
        (highlight_row == row_index).then(|| {
            let base = accent_bg.unwrap_or(if row_index.is_multiple_of(2) {
                theme.stripe
            } else {
                theme.panel
            });
            theme.row_highlight(base, strength)
        })
    });
    let cell_bg = highlight_bg.or(accent_bg);

    row.col(|ui| {
        if let Some(cell_bg) = cell_bg {
            paint_cell(ui, cell_bg);
        }
        ui.add(
            egui::Label::new(
                RichText::new(entry.line_no.to_string())
                    .monospace()
                    .color(theme.weak_text),
            )
            .truncate(),
        );
    });

    row.col(|ui| {
        if let Some(cell_bg) = cell_bg {
            paint_cell(ui, cell_bg);
        }
        if let Some(time) = entry.time {
            let response = ui.add(
                egui::Label::new(
                    RichText::new(time.to_string())
                        .monospace()
                        .color(theme.weak_text),
                )
                .truncate(),
            );
            response.on_hover_text(format!("{} {time}", time.date_string()));
        }
    });

    row.col(|ui| {
        if let Some(cell_bg) = cell_bg {
            paint_cell(ui, cell_bg);
        }
        if entry.continuation {
            ui.add(egui::Label::new(RichText::new("↳").color(theme.weak_text)).truncate());
        } else {
            ui.add(
                egui::Label::new(
                    RichText::new(entry.category.as_str())
                        .monospace()
                        .strong()
                        .color(theme.category_color(entry.category)),
                )
                .truncate(),
            );
        }
    });

    row.col(|ui| {
        if let Some(cell_bg) = cell_bg {
            paint_cell(ui, cell_bg);
        }
        render_message(ui, entry, log, theme, view, copied);
    });
}

/// Renders the message with ckey and character-name badges.
///
/// - Raw mode keeps the original `/`, parentheses and asterisks.
/// - `format` drops that syntax and uses the cleaned text.
/// - `dedup` collapses a character name that just repeats the same ckey.
fn render_message(
    ui: &mut Ui,
    entry: &LogRow,
    log: &LogFile,
    theme: &Theme,
    view: ViewOptions,
    copied: &mut Option<CopiedText>,
) {
    // Runtime-error blocks are shown exactly as logged.
    let format = view.format && !entry.runtime;
    let dedup = view.dedup && !entry.runtime;
    let (message, badges, marks) = if entry.runtime {
        (entry.message.as_str(), &[][..], &[][..])
    } else if (format || dedup)
        && let Some(formatted) = &entry.formatted
    {
        (
            formatted.text.as_str(),
            formatted.badges.as_slice(),
            formatted.marks.as_slice(),
        )
    } else {
        (
            entry.message.as_str(),
            entry.badges.as_slice(),
            entry.marks.as_slice(),
        )
    };

    let text_color = if entry.continuation {
        theme.weak_text
    } else {
        theme.text
    };

    let char_width = ui
        .ctx()
        .fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(12.0), '0'));
    let columns = log.format_columns;
    let (key_slot, name_slot) = if format {
        (
            columns.key_chars as f32 * char_width + 12.0 + 8.0,
            columns.name_chars as f32 * char_width + 24.0 + 8.0,
        )
    } else {
        (0.0, 0.0)
    };
    let admin_slots = if format {
        (
            columns.admin_ticket_chars as f32 * char_width + 12.0 + 8.0,
            columns.admin_sender_chars as f32 * char_width + 12.0 + 8.0,
            columns.admin_recipient_chars as f32 * char_width + 12.0 + 8.0,
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let admin_ticket = if format {
        entry
            .formatted
            .as_ref()
            .and_then(|formatted| formatted.admin_ticket)
    } else {
        None
    };
    let quote_admin = format
        && marks
            .iter()
            .any(|mark| matches!(mark.kind, MarkKind::AdminPm { .. }));

    ui.horizontal(|ui| {
        // Keep original spacing/punctuation tight around badges. Text
        // segments already contain the spaces from the log.
        ui.spacing_mut().item_spacing.x = 0.0;

        if entry.continuation {
            if format {
                // Line up detail lines with the message text that follows the
                // virtual ckey/name columns.
                ui.add_space(key_slot + name_slot);
            }
            ui.label(RichText::new("└ ").color(theme.weak_text));
        }

        let mut cursor = 0;
        let mut badge_index = 0;
        let mut mark_index = 0;
        let mut deferred_coords: Vec<Range<usize>> = Vec::new();

        while badge_index < badges.len() || mark_index < marks.len() {
            let take_mark = match (badges.get(badge_index), marks.get(mark_index)) {
                (Some(badge), Some(mark)) => mark.start < badge.key_start,
                (None, Some(_)) => true,
                _ => false,
            };

            if take_mark {
                let mark = &marks[mark_index];
                mark_index += 1;
                if mark.start < cursor || mark.end > message.len() || mark.start >= mark.end {
                    continue;
                }

                let mut start = mark.start;
                let mut end = mark.end;
                if format && matches!(mark.kind, MarkKind::Mob) {
                    // Format mode drops the parentheses around mob refs.
                    if start > cursor && message[..start].ends_with('(') {
                        start -= 1;
                    }
                    if message[end..].starts_with(')') {
                        end += 1;
                    }
                }

                if cursor < start {
                    text_segment(ui, &message[cursor..start], text_color);
                }
                match &mark.kind {
                    MarkKind::AsCharacter { .. } => {
                        let leading = format && !entry.continuation && mark.start == 0;
                        let columns = leading.then_some((key_slot, name_slot));
                        if let Some(coords) = render_as_character(
                            ui, message, mark, log, theme, view, text_color, copied, columns,
                        ) {
                            deferred_coords.push(coords);
                        }
                        cursor = end;
                        continue;
                    }
                    MarkKind::Character { key, name } => {
                        render_character(ui, message, name, *key, log, theme, copied);
                        cursor = end;
                        continue;
                    }
                    MarkKind::AdminPm {
                        ticket,
                        sender,
                        recipient,
                    } => {
                        render_admin_pm(
                            ui,
                            *ticket,
                            *sender,
                            *recipient,
                            log,
                            theme,
                            view,
                            text_color,
                            admin_slots,
                            copied,
                        );
                        cursor = end;
                        continue;
                    }
                    MarkKind::TicketRef { number } => {
                        render_ticket_badge(ui, *number, theme, copied);
                        cursor = end;
                        continue;
                    }
                    MarkKind::Sensitive { value } => {
                        if view.hide_sensitive {
                            let (fill, fill_text) = theme.system_badge_colors();
                            badge_button(ui, "hidden", fill, fill_text, false)
                                .on_hover_text("Sensitive data hidden");
                        } else {
                            text_segment(ui, &message[value.clone()], text_color);
                        }
                        cursor = end;
                        continue;
                    }
                    _ => {}
                }
                render_mark(ui, message, mark, theme, format, text_color, copied);
                cursor = end;
                continue;
            }

            let badge = &badges[badge_index];
            badge_index += 1;
            if badge.key_start < cursor || badge.key_end > message.len() {
                continue;
            }
            if cursor < badge.key_start {
                text_segment(ui, &message[cursor..badge.key_start], text_color);
            }

            let Some(player) = log.keys.get(badge.key as usize) else {
                cursor = badge.key_end;
                continue;
            };

            let is_leading =
                format && !entry.continuation && badge_index == 1 && badge.key_start == 0;
            let compact_admin = is_leading && admin_ticket.is_some();

            let block_start = ui.cursor().left();
            if compact_admin && let Some(number) = admin_ticket {
                render_ticket_badge(ui, number, theme, copied);
                let used = ui.cursor().left() - block_start;
                if used < admin_slots.0 {
                    ui.add_space(admin_slots.0 - used);
                }
            }

            let key_slot_start = ui.cursor().left();

            let concealed = view.hide_sensitive && !player.system;
            let (fill, badge_fg) = if player.system {
                theme.system_badge_colors()
            } else {
                theme.key_colors(badge.key as usize)
            };

            if concealed {
                badge_button(ui, "hidden", fill, badge_fg, false).on_hover_text("Key/ckey hidden");
            } else {
                let content = if player.system {
                    if format {
                        "no key"
                    } else {
                        &message[badge.key_start..badge.key_end]
                    }
                } else if format || dedup {
                    player.ckey.as_str()
                } else {
                    &message[badge.key_start..badge.key_end]
                };
                let hover = if player.system {
                    format!("{content}\nClick to copy")
                } else {
                    format!("key: {}\nckey: {}\nClick to copy", player.key, player.ckey)
                };

                copyable_badge(ui, content, (fill, badge_fg), false, copied, &hover);
            }

            if compact_admin {
                let used = ui.cursor().left() - key_slot_start;
                if used < admin_slots.1 {
                    ui.add_space(admin_slots.1 - used);
                }
                // Leave the arrow + recipient column (and the quote) empty so
                // the action text lines up with ticket message bodies.
                ui.add_space(char_width * 4.0 + admin_slots.2);
            } else if is_leading {
                let used = ui.cursor().left() - key_slot_start;
                if used < key_slot {
                    ui.add_space(key_slot - used);
                }
            }

            cursor = badge.key_end;

            if let Some(range) = &badge.name {
                let raw_name = &message[range.clone()];

                // Dedup: the mob name repeats the key/ckey of the same player
                // (`maleK1337/(Malek1337)`), so keep only the ckey badge.
                let name_hidden = dedup && player.matches_name(raw_name);
                let name_slot_start = ui.cursor().left();

                if compact_admin {
                    // Ticket actions: the ticket badge is shown first and the
                    // character name is omitted.
                    cursor = range.end + 1;
                    continue;
                }

                if !name_hidden {
                    if !format && cursor < range.start {
                        // Keep the original `/` and `(` around the badges.
                        text_segment(ui, &message[cursor..range.start], text_color);
                    } else if format && !is_leading {
                        text_segment(ui, " ", text_color);
                    }

                    let name = if format {
                        raw_name.trim_matches('*')
                    } else {
                        raw_name
                    };
                    copyable_badge(
                        ui,
                        name,
                        (fill, badge_fg),
                        true,
                        copied,
                        &format!("Character name: {name}\nClick to copy"),
                    );

                    cursor = if format { range.end + 1 } else { range.end };
                } else {
                    cursor = range.end + 1;
                }

                if is_leading {
                    // Include an immediately following mob ref in the same
                    // virtual column as the character name.
                    if let Some(mark) = marks.get(mark_index)
                        && matches!(mark.kind, MarkKind::Mob)
                        && mark.start >= cursor
                        && message[cursor..mark.start].chars().all(|character| {
                            character.is_whitespace() || "()[]{}".contains(character)
                        })
                    {
                        ui.add_space(4.0);
                        render_mark(ui, message, mark, theme, format, text_color, copied);
                        mark_index += 1;
                        cursor = mark.end;
                        if message[cursor..].starts_with(')') {
                            cursor += 1;
                        }
                    }

                    let used = ui.cursor().left() - name_slot_start;
                    if used < name_slot {
                        ui.add_space(name_slot - used);
                    }
                } else if name_hidden {
                    continue;
                }
            } else if is_leading {
                ui.add_space(name_slot);
            }
        }

        if cursor < message.len() {
            text_segment(ui, &message[cursor..], text_color);
        }

        // Format/dedup move `ckey (as Name at x,y,z)` coordinates to the end.
        for coords in deferred_coords {
            text_segment(ui, " ", text_color);
            copyable_badge(
                ui,
                &message[coords.clone()],
                theme.location_badge_colors(),
                false,
                copied,
                &format!("Coordinates: {}\nClick to copy", &message[coords]),
            );
        }

        if quote_admin {
            text_segment(ui, "\"", text_color);
        }
    });
}

/// `ckey (as Name at x,y,z)`. Returns the coordinates range when they should be
/// rendered at the end of the line (Format and Dedup modes).
#[allow(clippy::too_many_arguments)]
fn render_as_character(
    ui: &mut Ui,
    message: &str,
    mark: &Mark,
    log: &LogFile,
    theme: &Theme,
    view: ViewOptions,
    text_color: Color32,
    copied: &mut Option<CopiedText>,
    columns: Option<(f32, f32)>,
) -> Option<Range<usize>> {
    let MarkKind::AsCharacter {
        key,
        key_end,
        name,
        coords,
    } = &mark.kind
    else {
        return None;
    };
    let player = log.keys.get(*key as usize)?;
    let format = view.format;
    let (fill, badge_fg) = if player.system {
        theme.system_badge_colors()
    } else {
        theme.key_colors(*key as usize)
    };

    let key_x = ui.cursor().left();
    if view.hide_sensitive && !player.system {
        badge_button(ui, "hidden", fill, badge_fg, false).on_hover_text("Key/ckey hidden");
    } else {
        let raw_key = &message[mark.start..*key_end];
        let content = if player.system {
            if format { "no key" } else { raw_key }
        } else if format || view.dedup {
            player.ckey.as_str()
        } else {
            raw_key
        };
        let hover = if player.system {
            format!("{content}\nClick to copy")
        } else {
            format!("key: {}\nckey: {}\nClick to copy", player.key, player.ckey)
        };
        copyable_badge(ui, content, (fill, badge_fg), false, copied, &hover);
    }

    let name_text = &message[name.clone()];
    let name_hidden = view.dedup && player.matches_name(name_text);
    let defer = format || view.dedup;

    // Leading identity in Format mode: fill the virtual key and name columns.
    let (key_slot, name_slot) = columns.unwrap_or((0.0, 0.0));
    if columns.is_some() {
        let used = ui.cursor().left() - key_x;
        if used < key_slot {
            ui.add_space(key_slot - used);
        }
    }
    let name_x = ui.cursor().left();

    if !name_hidden {
        if format {
            if columns.is_none() {
                text_segment(ui, " ", text_color);
            }
        } else {
            text_segment(ui, &message[*key_end..name.start], text_color);
        }
        copyable_badge(
            ui,
            name_text,
            (fill, badge_fg),
            true,
            copied,
            &format!("Character name: {name_text}\nClick to copy"),
        );
    }

    if columns.is_some() {
        let used = ui.cursor().left() - name_x;
        if used < name_slot {
            ui.add_space(name_slot - used);
        }
    }

    if defer {
        return Some(coords.clone());
    }

    // Raw mode: keep every original character, only wrap the coordinates.
    text_segment(ui, &message[name.end..coords.start], text_color);
    copyable_badge(
        ui,
        &message[coords.clone()],
        theme.location_badge_colors(),
        false,
        copied,
        &format!("Coordinates: {}\nClick to copy", &message[coords.clone()]),
    );
    text_segment(ui, &message[coords.end..mark.end], text_color);
    None
}

/// A character name found in plain text (e.g. a salary line).
fn render_character(
    ui: &mut Ui,
    message: &str,
    name: &Range<usize>,
    key: u32,
    log: &LogFile,
    theme: &Theme,
    copied: &mut Option<CopiedText>,
) {
    let Some(player) = log.keys.get(key as usize) else {
        return;
    };
    let (fill, badge_fg) = if player.system {
        theme.system_badge_colors()
    } else {
        theme.key_colors(key as usize)
    };
    let name_text = &message[name.clone()];
    copyable_badge(
        ui,
        name_text,
        (fill, badge_fg),
        true,
        copied,
        &format!("Character name: {name_text}\nClick to copy"),
    );
}

fn render_ticket_badge(ui: &mut Ui, number: u32, theme: &Theme, copied: &mut Option<CopiedText>) {
    let colors = theme.chat_badge_colors(theme.category_color(Category::AdminPrivate));
    let label = format!("Ticket #{number}");
    copyable_badge(
        ui,
        &label,
        colors,
        false,
        copied,
        &format!("{label}\nClick to copy"),
    );
}

/// Renders the aligned admin PM header:
/// `[Ticket #N] [sender] → [recipient]: `.
#[allow(clippy::too_many_arguments)]
fn render_admin_pm(
    ui: &mut Ui,
    ticket: Option<u32>,
    sender: u32,
    recipient: u32,
    log: &LogFile,
    theme: &Theme,
    view: ViewOptions,
    text_color: Color32,
    slots: (f32, f32, f32),
    copied: &mut Option<CopiedText>,
) {
    let (ticket_slot, sender_slot, recipient_slot) = slots;

    let start = ui.cursor().left();
    if let Some(number) = ticket {
        render_ticket_badge(ui, number, theme, copied);
    }
    let used = ui.cursor().left() - start;
    if used < ticket_slot {
        ui.add_space(ticket_slot - used);
    }

    let start = ui.cursor().left();
    if let Some(player) = log.keys.get(sender as usize) {
        key_chip(
            ui,
            player,
            sender as usize,
            theme,
            view.hide_sensitive,
            copied,
        );
    }
    let used = ui.cursor().left() - start;
    if used < sender_slot {
        ui.add_space(sender_slot - used);
    }

    text_segment(ui, " → ", text_color);

    let start = ui.cursor().left();
    if let Some(player) = log.keys.get(recipient as usize) {
        key_chip(
            ui,
            player,
            recipient as usize,
            theme,
            view.hide_sensitive,
            copied,
        );
    }
    let used = ui.cursor().left() - start;
    if used < recipient_slot {
        ui.add_space(recipient_slot - used);
    }
    text_segment(ui, " \"", text_color);
}

fn render_mark(
    ui: &mut Ui,
    message: &str,
    mark: &Mark,
    theme: &Theme,
    format: bool,
    text_color: Color32,
    copied: &mut Option<CopiedText>,
) {
    match &mark.kind {
        MarkKind::Mob => {
            let content = &message[mark.start..mark.end];
            copyable_badge(
                ui,
                content,
                theme.mob_badge_colors(),
                false,
                copied,
                &format!("Entity: {content}\nClick to copy"),
            );
        }
        MarkKind::AsCharacter { .. } | MarkKind::Character { .. } => {}
        MarkKind::AdminPm { .. } | MarkKind::TicketRef { .. } | MarkKind::Sensitive { .. } => {}
        MarkKind::Location { name, coords } => {
            let name_text = &message[name.clone()];
            let coords_text = &message[coords.clone()];
            let colors = theme.location_badge_colors();

            if !format {
                // Plain text: `(Location (x,y,z))`
                text_segment(ui, "(", text_color);
            }

            copyable_badge(
                ui,
                name_text,
                colors,
                false,
                copied,
                &format!("Location: {name_text}\nClick to copy"),
            );

            if !format {
                text_segment(ui, " (", text_color);
            } else {
                text_segment(ui, " ", text_color);
            }

            copyable_badge(
                ui,
                coords_text,
                colors,
                false,
                copied,
                &format!("Coordinates: {coords_text}\nClick to copy"),
            );

            if !format {
                text_segment(ui, "))", text_color);
            }
        }
        MarkKind::Frequency { value, spans } => {
            let value_text = &message[value.clone()];
            let spans_text = spans.as_ref().map(|range| &message[range.clone()]);
            let channel = crate::radio::channel_for(spans_text, value_text);
            let colors = theme.chat_badge_colors(crate::radio::color_for(spans_text, value_text));
            let tooltip = match channel {
                Some(channel) => format!(
                    "{} radio channel ({value_text})\nClick to copy",
                    channel.name
                ),
                None => format!("Radio frequency {value_text}\nClick to copy"),
            };

            if !format {
                // Plain text: `[135.9] (spans: …)`
                text_segment(ui, &message[mark.start..value.start], text_color);
            }

            copyable_badge(ui, value_text, colors, false, copied, &tooltip);

            if !format {
                // Plain text keeps `]` and the whole `(spans: …)` tail.
                text_segment(ui, &message[value.end..mark.end], text_color);
            }
        }
    }
}

fn text_segment(ui: &mut Ui, text: &str, color: Color32) {
    if text.is_empty() {
        return;
    }
    ui.add(
        egui::Label::new(RichText::new(text).color(color)).wrap_mode(egui::TextWrapMode::Extend),
    );
}

fn copyable_badge(
    ui: &mut Ui,
    content: &str,
    colors: (Color32, Color32),
    round: bool,
    copied: &mut Option<CopiedText>,
    hover: &str,
) {
    let now = ui.input(|input| input.time);
    let (fill, text_color) = colors;
    let is_copied = copied
        .as_ref()
        .is_some_and(|copied| copied.text == content && now < copied.until);
    let label = if is_copied { "copied!" } else { content };

    let response = badge_button(ui, label, fill, text_color, round).on_hover_text(hover);
    if response.clicked() {
        ui.ctx().copy_text(content.to_owned());
        *copied = Some(CopiedText {
            text: content.to_owned(),
            until: now + 1.0,
        });
        ui.ctx().request_repaint();
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }
}

/// A chip that looks like a badge but is made of real, selectable text:
/// dragging across it selects the content together with the surrounding text.
/// Clicking still reports a click so the caller can copy on demand.
fn badge_button(
    ui: &mut Ui,
    text: &str,
    fill: Color32,
    text_color: Color32,
    round: bool,
) -> egui::Response {
    const PADDING: egui::Vec2 = egui::vec2(6.0, 1.0);

    let font_id = egui::FontId::monospace(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font_id, text_color);

    let (rect, response) = ui.allocate_at_least(galley.size() + 2.0 * PADDING, Sense::hover());

    let corner_radius = if round {
        CornerRadius::same(9)
    } else {
        CornerRadius::same(4)
    };
    ui.painter().rect_filled(rect, corner_radius, fill);

    let text_rect = egui::Rect::from_min_size(rect.min + PADDING, galley.size());
    let label = egui::Label::new(RichText::new(text).monospace().size(12.0).color(text_color))
        .selectable(true)
        .wrap_mode(egui::TextWrapMode::Extend);

    response.union(ui.put(text_rect, label))
}

/// Paints a cell background, including half the inter-cell gap, so adjacent
/// cells form a continuous band.
fn paint_cell(ui: &Ui, color: Color32) {
    let rect = ui
        .max_rect()
        .expand2(0.5 * ui.spacing().item_spacing + egui::vec2(0.5, 0.5));
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Paints the accent for a whole table row in one go, so no panel-colored gaps
/// remain between cells or rows.
fn rows_label(count: usize) -> String {
    format!(
        "{} {}",
        group_digits(count),
        if count == 1 { "row" } else { "rows" }
    )
}

fn entries_label(count: usize) -> String {
    format!(
        "{} {}",
        group_digits(count),
        if count == 1 { "entry" } else { "entries" }
    )
}

fn group_digits(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push('\u{00A0}');
        }
        out.push(digit);
    }
    out
}

struct FilterState {
    open: bool,
    enabled: [bool; Category::COUNT],
    /// Original row indices after category filters (and Dedup).
    visible: Vec<u32>,
    dirty: bool,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            open: false,
            enabled: [true; Category::COUNT],
            visible: Vec::new(),
            dirty: true,
        }
    }
}

#[derive(Default)]
struct ListState {
    ckeys: bool,
    names: bool,
    pairs: bool,
}

fn player_colors(theme: &Theme, log: &LogFile, key: u32) -> (Color32, Color32) {
    match log.keys.get(key as usize) {
        Some(player) if !player.system => theme.key_colors(key as usize),
        _ => theme.system_badge_colors(),
    }
}

/// Ckey chip for the list windows: copyable, or a neutral "hidden" badge when
/// Hide sensitive is on.
fn key_chip(
    ui: &mut Ui,
    player: &crate::log::PlayerKey,
    index: usize,
    theme: &Theme,
    hide_sensitive: bool,
    copied: &mut Option<CopiedText>,
) {
    let (fill, text) = if player.system {
        theme.system_badge_colors()
    } else {
        theme.key_colors(index)
    };
    if hide_sensitive && !player.system {
        badge_button(ui, "hidden", fill, text, false).on_hover_text("Key/ckey hidden");
        return;
    }
    let hover = format!("key: {}\nckey: {}\nClick to copy", player.key, player.ckey);
    copyable_badge(ui, &player.ckey, (fill, text), false, copied, &hover);
}
