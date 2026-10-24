use super::*;

pub(super) fn row_matches(
    row: &LogRow,
    query_lower: &str,
    keys: &[PlayerKey],
    dedup: bool,
) -> bool {
    if dedup {
        // In Dedup mode only canonical ckeys are shown
        return dedup_display_text(row, keys)
            .to_lowercase()
            .contains(query_lower)
            || row.category.as_str().to_lowercase().contains(query_lower);
    }

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

/// A row's text with every key badge replaced by the canonical ckey, matching
/// what the table shows in Dedup mode.
pub(super) fn dedup_display_text(row: &LogRow, keys: &[PlayerKey]) -> String {
    let (text, badges, marks) = match &row.formatted {
        Some(formatted) => (
            formatted.text.as_str(),
            formatted.badges.as_slice(),
            formatted.marks.as_slice(),
        ),
        None => (
            row.message.as_str(),
            row.badges.as_slice(),
            row.marks.as_slice(),
        ),
    };

    let mut replacements: Vec<(usize, usize, &str)> = badges
        .iter()
        .filter_map(|badge| {
            keys.get(badge.key as usize)
                .map(|player| (badge.key_start, badge.key_end, player.ckey.as_str()))
        })
        .collect();

    for mark in marks {
        if let MarkKind::AsCharacter { key, key_end, .. } = &mark.kind
            && let Some(player) = keys.get(*key as usize)
        {
            replacements.push((mark.start, *key_end, player.ckey.as_str()));
        }
    }

    replacements.sort_by_key(|(start, _, _)| *start);

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end, ckey) in replacements {
        if start < cursor || end > text.len() {
            continue;
        }
        out.push_str(&text[cursor..start]);
        out.push_str(ckey);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

pub(super) fn row_matches_identity(
    row: &LogRow,
    selected_keys: &[(u32, String, String)],
    selected_names: &HashSet<String>,
    mode: IdentityMode,
) -> bool {
    if selected_keys.is_empty() && selected_names.is_empty() {
        return true;
    }

    let mut representations: Vec<(&str, &[crate::log::TextBadge], &[Mark])> =
        vec![(&row.message, &row.badges, &row.marks)];
    if let Some(formatted) = &row.formatted {
        representations.push((&formatted.text, &formatted.badges, &formatted.marks));
    }

    let has_key = |key: u32, ckey: &str, display_key: &str| {
        representations.iter().any(|(text, badges, marks)| {
            if badges.iter().any(|badge| badge.key == key) {
                return true;
            }
            if marks.iter().any(|mark| match &mark.kind {
                MarkKind::AsCharacter { key: mark_key, .. }
                | MarkKind::Character { key: mark_key, .. } => *mark_key == key,
                MarkKind::AdminPm {
                    sender, recipient, ..
                } => *sender == key || *recipient == key,
                _ => false,
            }) {
                return true;
            }

            let lower = text.to_lowercase();
            contains_word(&lower, ckey) || contains_word(&lower, display_key)
        })
    };

    let has_name = |name: &str| {
        representations.iter().any(|(text, badges, marks)| {
            if badges.iter().any(|badge| {
                badge
                    .name
                    .as_ref()
                    .is_some_and(|range| &text[range.clone()] == name)
            }) {
                return true;
            }
            if marks.iter().any(|mark| match &mark.kind {
                MarkKind::AsCharacter { name: range, .. }
                | MarkKind::Character { name: range, .. } => &text[range.clone()] == name,
                _ => false,
            }) {
                return true;
            }
            text.to_lowercase().contains(&name.to_lowercase())
        })
    };

    match mode {
        IdentityMode::Add => {
            selected_keys
                .iter()
                .any(|(key, ckey, display)| has_key(*key, ckey, display))
                || selected_names.iter().any(|name| has_name(name))
        }
        IdentityMode::Overlap => {
            selected_keys
                .iter()
                .all(|(key, ckey, display)| has_key(*key, ckey, display))
                && selected_names.iter().all(|name| has_name(name))
        }
    }
}

pub(super) fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }

    let mut offset = 0;
    while let Some(position) = haystack[offset..].find(needle) {
        let start = offset + position;
        let end = start + needle.len();
        let before = haystack[..start].chars().next_back();
        let after = haystack[end..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|value| !value.is_alphanumeric() && value != '_')
        };
        if boundary(before) && boundary(after) {
            return true;
        }
        offset = end;
    }
    false
}

pub(super) fn identity_mode_toggle(ui: &mut Ui, mode: &mut IdentityMode, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Combine:").weak());
        let before = *mode;
        ui.selectable_value(mode, IdentityMode::Add, "Add");
        ui.selectable_value(mode, IdentityMode::Overlap, "Overlap");
        if *mode != before {
            *changed = true;
        }
    });
}
impl RlogisApp {
    pub(super) fn filters_changed(&mut self) {
        self.filters.dirty = true;
        self.search.active_query.clear();
        self.search.matches.clear();
        self.search.scroll_to = None;
    }

    pub(super) fn refresh_visible_rows(&mut self) {
        if !self.filters.dirty {
            return;
        }

        let enabled = self.filters.enabled;
        let selected_names = self.filters.selected_names.clone();
        let identity_mode = self.filters.identity_mode;
        let selected_keys: Vec<(u32, String, String)> = self
            .filters
            .selected_keys
            .iter()
            .filter_map(|key| {
                self.log
                    .as_ref()
                    .and_then(|log| log.keys.get(*key as usize))
                    .map(|player| (*key, player.ckey.clone(), player.key.to_lowercase()))
            })
            .collect();
        let identity_filter = !selected_keys.is_empty() || !selected_names.is_empty();
        let dedup = self.dedup;
        let mut visible = Vec::new();

        if let Some(log) = &self.log {
            let keep = |index: u32| {
                let row = &log.rows[index as usize];
                enabled[row.category.index()]
                    && (!identity_filter
                        || row_matches_identity(
                            row,
                            &selected_keys,
                            &selected_names,
                            identity_mode,
                        ))
            };

            if dedup {
                for &index in &log.dedup_indices {
                    if keep(index) {
                        visible.push(index);
                    }
                }
            } else {
                for index in 0..log.rows.len() as u32 {
                    if keep(index) {
                        visible.push(index);
                    }
                }
            }
        }

        self.filters.visible = visible;
        self.filters.dirty = false;
    }

    pub(super) fn filter_window(&mut self, ctx: &egui::Context) {
        if !self.filters.open {
            return;
        }

        let theme = self.theme.theme();
        let mut open = true;
        let mut changed = false;
        let content = ctx.content_rect();
        let position = egui::pos2(content.left() + 80.0, content.top() + 100.0);

        egui::Window::new("Types")
            .open(&mut open)
            .default_pos(position)
            .movable(true)
            .collapsible(false)
            .resizable(false)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.filters.type_search)
                        .hint_text("Search types…"),
                );
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

                let query = self.filters.type_search.to_lowercase();
                let mut shown = 0;
                for category in Category::ALL {
                    if !query.is_empty() && !category.as_str().to_lowercase().contains(&query) {
                        continue;
                    }
                    shown += 1;
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
                if shown == 0 {
                    ui.label(RichText::new("No matches").weak());
                }
            });

        self.filters.open = open;
        if changed {
            self.filters_changed();
        }
    }
}
