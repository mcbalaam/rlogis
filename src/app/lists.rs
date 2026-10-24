use super::filters::identity_mode_toggle;
use super::*;

impl RlogisApp {
    pub(super) fn list_windows(&mut self, ctx: &egui::Context) {
        if !(self.lists.ckeys || self.lists.names) {
            return;
        }

        let theme = self.theme.theme();
        let content = ctx.content_rect();
        let base = egui::pos2(content.left() + 60.0, content.top() + 90.0);

        let (mut ckeys, mut names) = {
            let Some(log) = self.log.as_ref() else {
                return;
            };
            let ckeys: Vec<(u32, String)> = log
                .keys
                .iter()
                .enumerate()
                .filter(|(_, player)| !player.system)
                .map(|(index, player)| (index as u32, player.ckey.clone()))
                .collect();
            let names: Vec<(String, u32)> = log
                .characters
                .iter()
                .map(|character| (character.name.clone(), character.key))
                .collect();
            (ckeys, names)
        };
        ckeys.sort_by(|a, b| a.1.cmp(&b.1));
        names.sort_by_key(|entry| entry.0.to_lowercase());
        names.dedup_by(|a, b| a.0 == b.0);

        let mut changed = false;

        if self.lists.ckeys {
            let mut open = true;
            egui::Window::new("Ckeys")
                .open(&mut open)
                .default_pos(base)
                .default_width(340.0)
                .max_width(360.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(320.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.lists.ckeys_search)
                            .hint_text("Search ckeys…"),
                    );
                    identity_mode_toggle(ui, &mut self.filters.identity_mode, &mut changed);
                    ui.separator();

                    let query = self.lists.ckeys_search.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            let mut shown = 0;
                            for (index, ckey) in &ckeys {
                                if !query.is_empty() && !ckey.to_lowercase().contains(&query) {
                                    continue;
                                }
                                shown += 1;
                                let mut checked = self.filters.selected_keys.contains(index);
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut checked, "").changed() {
                                        if checked {
                                            self.filters.selected_keys.insert(*index);
                                        } else {
                                            self.filters.selected_keys.remove(index);
                                        }
                                        changed = true;
                                    }
                                    let (_, color) = theme.key_colors(*index as usize);
                                    let label = if self.hide_sensitive {
                                        "hidden"
                                    } else {
                                        ckey.as_str()
                                    };
                                    ui.label(RichText::new(label).monospace().color(color));
                                });
                            }
                            if shown == 0 {
                                ui.label(RichText::new("No matches").weak());
                            }
                        });
                });
            self.lists.ckeys = open;
        }

        if self.lists.names {
            let mut open = true;
            egui::Window::new("Names")
                .open(&mut open)
                .default_pos(base + egui::vec2(30.0, 30.0))
                .default_width(340.0)
                .max_width(360.0)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.set_max_width(320.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.lists.names_search)
                            .hint_text("Search names…"),
                    );
                    identity_mode_toggle(ui, &mut self.filters.identity_mode, &mut changed);
                    ui.separator();

                    let query = self.lists.names_search.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            let mut shown = 0;
                            for (name, key) in &names {
                                if !query.is_empty() && !name.to_lowercase().contains(&query) {
                                    continue;
                                }
                                shown += 1;
                                let mut checked = self.filters.selected_names.contains(name);
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut checked, "").changed() {
                                        if checked {
                                            self.filters.selected_names.insert(name.clone());
                                        } else {
                                            self.filters.selected_names.remove(name);
                                        }
                                        changed = true;
                                    }
                                    let (_, color) = theme.key_colors(*key as usize);
                                    ui.label(RichText::new(name).color(color));
                                });
                            }
                            if shown == 0 {
                                ui.label(RichText::new("No matches").weak());
                            }
                        });
                });
            self.lists.names = open;
        }

        if changed {
            self.filters_changed();
        }
    }
}
