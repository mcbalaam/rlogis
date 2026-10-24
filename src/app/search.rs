use super::filters::row_matches;
use super::*;

impl RlogisApp {
    pub(super) fn search_window(&mut self, ctx: &egui::Context) {
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

    pub(super) fn submit_search(&mut self) {
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
            if row_matches(
                &log.rows[original_index as usize],
                &query_lower,
                &log.keys,
                self.dedup,
            ) {
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

    pub(super) fn step_search(&mut self, step: isize) {
        if self.search.matches.is_empty() {
            return;
        }

        let len = self.search.matches.len() as isize;
        self.search.current = (self.search.current as isize + step).rem_euclid(len) as usize;
        self.search.scroll_to = self.search.matches.get(self.search.current).copied();
    }
}
