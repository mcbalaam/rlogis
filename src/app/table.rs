use super::badges::*;
use super::*;

pub(super) fn table_ui(
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

pub(super) fn render_row(
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
pub(super) fn render_message(
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
                } else if view.dedup {
                    format!("ckey: {}\nClick to copy", player.ckey)
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
pub(super) fn render_as_character(
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
        } else if view.dedup {
            format!("ckey: {}\nClick to copy", player.ckey)
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
pub(super) fn render_character(
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

/// Renders the aligned admin PM header:
/// `[Ticket #N] [sender] → [recipient]: `.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_admin_pm(
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
            view.dedup,
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
            view.dedup,
            copied,
        );
    }
    let used = ui.cursor().left() - start;
    if used < recipient_slot {
        ui.add_space(recipient_slot - used);
    }
    text_segment(ui, " \"", text_color);
}

pub(super) fn render_mark(
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

pub(super) fn text_segment(ui: &mut Ui, text: &str, color: Color32) {
    if text.is_empty() {
        return;
    }
    ui.add(
        egui::Label::new(RichText::new(text).color(color)).wrap_mode(egui::TextWrapMode::Extend),
    );
}

/// Paints a cell background, including half the inter-cell gap, so adjacent
/// cells form a continuous band.
pub(super) fn paint_cell(ui: &Ui, color: Color32) {
    let rect = ui
        .max_rect()
        .expand2(0.5 * ui.spacing().item_spacing + egui::vec2(0.5, 0.5));
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// Paints the accent for a whole table row in one go, so no panel-colored gaps
/// remain between cells or rows.
pub(super) fn rows_label(count: usize) -> String {
    format!(
        "{} {}",
        group_digits(count),
        if count == 1 { "row" } else { "rows" }
    )
}

pub(super) fn entries_label(count: usize) -> String {
    format!(
        "{} {}",
        group_digits(count),
        if count == 1 { "entry" } else { "entries" }
    )
}

pub(super) fn group_digits(value: usize) -> String {
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
