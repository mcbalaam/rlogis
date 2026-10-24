use super::table::{entries_label, rows_label};
use super::*;

impl RlogisApp {
    pub(super) fn toolbar(&mut self, ui: &mut Ui) {
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
                        .on_hover_text("Filter by player key");
                    ui.toggle_value(&mut self.lists.names, "Names")
                        .on_hover_text("Filter by character name");
                    ui.toggle_value(&mut self.filters.open, "Types")
                        .on_hover_text("Hide log categories");

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
                        .on_hover_text("Collapse duplicates")
                        .changed()
                    {
                        self.filters.dirty = true;
                        self.search.active_query.clear();
                        self.search.matches.clear();
                        self.search.scroll_to = None;
                    }
                    ui.checkbox(&mut self.format, "Format")
                        .on_hover_text("Clean log syntax");
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
}
