//! Application state and UI.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{Align, Color32, CornerRadius, Layout, RichText, Ui};
use egui_extras::{Column, TableBuilder, TableRow};

use crate::log::{LogFile, LogRow, Mark, MarkKind};
use crate::platform::{self, PickOutcome};
use crate::theme::{Theme, ThemeKind};

const THEME_KEY: &str = "rlogis.theme";
const ACCENT_KEY: &str = "rlogis.row_accent";
const DEDUP_KEY: &str = "rlogis.dedup";
const FORMAT_KEY: &str = "rlogis.format";
const ROW_HEIGHT: f32 = 22.0;
const HEADER_HEIGHT: f32 = 27.0;

const OPEN_LOG: &str = "Open";
const OPEN_ANOTHER_LOG: &str = "Open another log";
const OPENING_FILE: &str = "Opening file…";
const OPEN_FAILED: &str = "Could not open the file";

pub struct RlogisApp {
    theme: ThemeKind,
    applied_theme: Option<ThemeKind>,
    row_accent: bool,
    dedup: bool,
    format: bool,
    log: Option<LogFile>,
    source: Option<String>,
    picker: Option<Receiver<PickOutcome>>,
    error: Option<String>,
    copied: Option<CopiedText>,
}

struct CopiedText {
    text: String,
    until: f64,
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

        theme.theme().apply(&cc.egui_ctx);

        Self {
            theme,
            applied_theme: Some(theme),
            row_accent,
            dedup,
            format,
            log: None,
            source: None,
            picker: None,
            error: None,
            copied: None,
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
                    PickOutcome::Loaded { name, bytes } => {
                        let text = String::from_utf8_lossy(&bytes);
                        self.log = Some(LogFile::parse(&text));
                        self.source = Some(name);
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
                    let open_label = OPEN_LOG;
                    if ui
                        .add_enabled(!picking, egui::Button::new(open_label))
                        .clicked()
                    {
                        self.open_picker(ui.ctx());
                    }

                    if picking {
                        ui.spinner();
                        ui.label(RichText::new(OPENING_FILE).weak());
                    }

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
                    ui.checkbox(&mut self.dedup, "Dedup").on_hover_text(
                        "Collapse duplicate key/name badges into one canonical ckey",
                    );
                    ui.checkbox(&mut self.format, "Format").on_hover_text(
                        "Clean log syntax and HTML: ckey, character name, then the message",
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(log) = &self.log {
                            let entries = entries_label(log.entry_count);
                            let rows = rows_label(log.rows.len());
                            ui.label(RichText::new(format!("{entries} · {rows}")).weak());
                            ui.separator();

                            if let Some(source) = &self.source {
                                let response =
                                    ui.label(RichText::new(source).strong().color(theme.text));
                                if let Some((first, last)) = log.time_range() {
                                    response.on_hover_text(format!(
                                        "{} {} – {}",
                                        first.date_string(),
                                        first,
                                        last
                                    ));
                                }
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
            let has_rows = self.log.as_ref().is_some_and(|log| !log.rows.is_empty());

            if has_rows {
                let log = self.log.as_ref().expect("checked above");
                table_ui(
                    ui,
                    log,
                    self.theme,
                    ViewOptions {
                        accent: self.row_accent,
                        dedup: self.dedup,
                        format: self.format,
                    },
                    &mut self.copied,
                );
            };
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

        let now = ctx.input(|input| input.time);
        if self
            .copied
            .as_ref()
            .is_some_and(|copied| now >= copied.until)
        {
            self.copied = None;
        }

        self.toolbar(ui);
        self.error_banner(ui);
        self.central(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.code().to_owned());
        storage.set_string(
            ACCENT_KEY,
            if self.row_accent { "1" } else { "0" }.to_owned(),
        );
        storage.set_string(DEDUP_KEY, if self.dedup { "1" } else { "0" }.to_owned());
        storage.set_string(FORMAT_KEY, if self.format { "1" } else { "0" }.to_owned());
    }
}

#[derive(Clone, Copy)]
struct ViewOptions {
    accent: bool,
    dedup: bool,
    format: bool,
}

fn table_ui(
    ui: &mut Ui,
    log: &LogFile,
    theme_kind: ThemeKind,
    view: ViewOptions,
    copied: &mut Option<CopiedText>,
) {
    let theme = theme_kind.theme();

    TableBuilder::new(ui)
        .id_salt("log_table")
        .striped(!view.accent)
        .resizable(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(86.0))
        .column(Column::exact(112.0))
        .column(Column::exact(124.0))
        .column(Column::remainder().clip(true))
        .min_scrolled_height(0.0)
        .auto_shrink([false, false])
        .header(HEADER_HEIGHT, |mut header| {
            for label in ["#", "Time", "Type", "Message"] {
                header.col(|ui| {
                    paint_cell(ui, theme.header);
                    ui.label(RichText::new(label).strong().color(theme.text));
                });
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, log.rows.len(), |mut row| {
                let entry = &log.rows[row.index()];
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
    let tint = view
        .accent
        .then(|| theme.row_accent(entry.category, entry.continuation));

    row.col(|ui| {
        if let Some(tint) = tint {
            paint_row_band(ui, tint);
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
    let format = view.format;
    let dedup = view.dedup;
    let (message, badges, marks) = if (format || dedup)
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

    ui.horizontal(|ui| {
        // Keep original spacing/punctuation tight around badges. Text
        // segments already contain the spaces from the log.
        ui.spacing_mut().item_spacing.x = 0.0;

        if entry.continuation {
            ui.label(RichText::new("└ ").color(theme.weak_text));
        }

        let mut cursor = 0;
        let mut badge_index = 0;
        let mut mark_index = 0;

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

            let (fill, badge_fg) = if player.system {
                theme.system_badge_colors()
            } else {
                theme.key_colors(badge.key as usize)
            };
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

            cursor = badge.key_end;

            if let Some(range) = &badge.name {
                let raw_name = &message[range.clone()];

                // Dedup: the mob name repeats the key/ckey of the same player
                // (`maleK1337/(Malek1337)`), so keep only the ckey badge.
                if dedup && player.matches_name(raw_name) {
                    cursor = range.end + 1;
                    continue;
                }

                if !format && cursor < range.start {
                    // Keep the original `/` and `(` around the badges.
                    text_segment(ui, &message[cursor..range.start], text_color);
                } else if format {
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
            }
        }

        if cursor < message.len() {
            text_segment(ui, &message[cursor..], text_color);
        }
    });
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

fn badge_button(
    ui: &mut Ui,
    text: &str,
    fill: Color32,
    text_color: Color32,
    round: bool,
) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(6.0, 1.0);
        ui.spacing_mut().interact_size.y = 16.0;
        let corner_radius = if round {
            CornerRadius::same(9)
        } else {
            CornerRadius::same(4)
        };
        ui.add(
            egui::Button::new(RichText::new(text).monospace().size(12.0).color(text_color))
                .fill(fill)
                .corner_radius(corner_radius)
                .stroke(egui::Stroke::NONE)
                .small(),
        )
    })
    .inner
}

/// Paints a cell background, including half the inter-cell gap, so adjacent
/// cells form a continuous band.
fn paint_cell(ui: &Ui, color: Color32) {
    let rect = ui.max_rect().expand2(0.5 * ui.spacing().item_spacing);
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Paints the accent for a whole table row in one go, so no panel-colored gaps
/// remain between cells or rows.
fn paint_row_band(ui: &Ui, color: Color32) {
    let cell = ui.max_rect();
    let spacing = ui.spacing().item_spacing;
    // Clip to the table's scroll viewport so bands never paint over the
    // sticky header, but fill the whole row width.
    let clip = ui.clip_rect();
    let rect = egui::Rect::from_min_max(
        egui::pos2(clip.left(), cell.top() - 1.0),
        egui::pos2(clip.right(), cell.bottom() + spacing.y + 1.0),
    );
    let mut painter = ui.painter().clone();
    painter.set_clip_rect(clip);
    painter.rect_filled(rect, CornerRadius::ZERO, color);
}

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
