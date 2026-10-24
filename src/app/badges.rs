use super::*;

pub(super) fn copyable_badge(
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

/// A colored chip
pub(super) fn badge_button(
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

/// Ckey chip
pub(super) fn key_chip(
    ui: &mut Ui,
    player: &crate::log::PlayerKey,
    index: usize,
    theme: &Theme,
    hide_sensitive: bool,
    dedup: bool,
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
    let hover = if dedup {
        format!("ckey: {}\nClick to copy", player.ckey)
    } else {
        format!("key: {}\nckey: {}\nClick to copy", player.key, player.ckey)
    };
    copyable_badge(ui, &player.ckey, (fill, text), false, copied, &hover);
}

pub(super) fn render_ticket_badge(
    ui: &mut Ui,
    number: u32,
    theme: &Theme,
    copied: &mut Option<CopiedText>,
) {
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
