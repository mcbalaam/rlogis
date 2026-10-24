use egui::{Color32, Stroke};

use crate::log::Category;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeKind {
    #[default]
    Light,
    Dark,
}

pub struct Theme {
    pub dark: bool,
    pub panel: Color32,
    pub top_bar: Color32,
    pub header: Color32,
    pub stripe: Color32,
    pub text: Color32,
    pub weak_text: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub danger: Color32,
    palette: [Color32; Category::COUNT],
}

impl Theme {
    pub fn category_color(&self, category: Category) -> Color32 {
        self.palette[category.index()]
    }

    pub fn row_accent(&self, category: Category, continuation: bool) -> Color32 {
        let strength = if self.dark { 0.14 } else { 0.12 };
        let strength = if continuation {
            strength * 0.55
        } else {
            strength
        };
        mix(self.panel, self.category_color(category), strength)
    }

    pub fn key_colors(&self, index: usize) -> (Color32, Color32) {
        let hue = (index as f32 * 137.508) % 360.0;
        let (saturation, lightness) = if self.dark {
            (0.42, 0.25)
        } else {
            (0.48, 0.86)
        };
        let background = hsl_to_rgb(hue, saturation, lightness);
        let text = if self.dark {
            self.text
        } else {
            Color32::from_rgb(35, 25, 32)
        };
        (background, text)
    }

    /// Gray badge colors for the `*no key*` pseudo-player and its mob name.
    pub fn system_badge_colors(&self) -> (Color32, Color32) {
        if self.dark {
            (
                Color32::from_rgb(42, 46, 54),
                Color32::from_rgb(208, 213, 222),
            )
        } else {
            (
                Color32::from_rgb(238, 240, 244),
                Color32::from_rgb(85, 92, 105),
            )
        }
    }

    /// Neutral slate badge for `mob_1234` references.
    pub fn mob_badge_colors(&self) -> (Color32, Color32) {
        if self.dark {
            (
                Color32::from_rgb(36, 41, 50),
                Color32::from_rgb(203, 210, 222),
            )
        } else {
            (
                Color32::from_rgb(238, 241, 245),
                Color32::from_rgb(60, 70, 85),
            )
        }
    }

    /// Blue badges for location names and coordinates.
    pub fn location_badge_colors(&self) -> (Color32, Color32) {
        if self.dark {
            (
                Color32::from_rgb(26, 42, 61),
                Color32::from_rgb(198, 220, 244),
            )
        } else {
            (
                Color32::from_rgb(233, 242, 252),
                Color32::from_rgb(35, 65, 95),
            )
        }
    }

    /// Badge colors derived from an in-game chat color (radio channels)
    pub fn chat_badge_colors(&self, chat: Color32) -> (Color32, Color32) {
        if self.dark {
            (mix(self.panel, chat, 0.34), mix(self.text, chat, 0.55))
        } else {
            (mix(self.panel, chat, 0.18), mix(chat, Color32::BLACK, 0.12))
        }
    }

    /// Amber flash used to highlight the row a search jumped to
    pub fn highlight_color(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(250, 204, 21)
        } else {
            Color32::from_rgb(245, 158, 11)
        }
    }

    /// Blend a row background toward the search-highlight color
    pub fn row_highlight(&self, base: Color32, strength: f32) -> Color32 {
        mix(
            base,
            self.highlight_color(),
            0.45 * strength.clamp(0.0, 1.0),
        )
    }

    /// Applies the theme to egui. Safe to call on every theme change
    pub fn apply(&self, ctx: &egui::Context) {
        let target = if self.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.panel;
        visuals.window_fill = self.panel;
        visuals.faint_bg_color = self.stripe;
        visuals.striped = true;
        visuals.hyperlink_color = self.accent;
        visuals.selection.bg_fill = self.accent.gamma_multiply(0.30);
        visuals.selection.stroke = Stroke::new(1.0, self.accent);

        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
        visuals.widgets.noninteractive.bg_fill = self.panel;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        visuals.widgets.inactive.bg_fill = self.top_bar;
        visuals.widgets.inactive.weak_bg_fill = self.top_bar;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        visuals.widgets.hovered.bg_fill = mix(self.panel, self.accent, 0.16);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.text);
        visuals.widgets.active.bg_stroke = Stroke::NONE;
        visuals.widgets.active.bg_fill = mix(self.panel, self.accent, 0.26);

        ctx.set_theme(target);
        ctx.set_visuals_of(target, visuals);
        ctx.all_styles_mut(|style| {
            style.spacing.button_padding = egui::vec2(10.0, 5.0);
            style.spacing.item_spacing = egui::vec2(8.0, 4.0);
            style.spacing.interact_size.y = 24.0;
        });
    }
}

impl ThemeKind {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    pub const fn code(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub const fn theme(self) -> &'static Theme {
        match self {
            Self::Light => &LIGHT,
            Self::Dark => &DARK,
        }
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    let to_u8 = |value: f32| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(to_u8(r), to_u8(g), to_u8(b))
}

static LIGHT: Theme = Theme {
    dark: false,
    panel: Color32::from_rgb(255, 255, 255),
    top_bar: Color32::from_rgb(244, 246, 249),
    header: Color32::from_rgb(238, 242, 247),
    stripe: Color32::from_rgb(237, 241, 245),
    text: Color32::from_rgb(31, 35, 41),
    weak_text: Color32::from_rgb(106, 115, 128),
    accent: Color32::from_rgb(59, 130, 246),
    accent_text: Color32::from_rgb(255, 255, 255),
    danger: Color32::from_rgb(185, 28, 28),
    palette: [
        Color32::from_rgb(100, 116, 139), // MISC
        Color32::from_rgb(14, 116, 144),  // SAY
        Color32::from_rgb(220, 38, 38),   // ATTACK
        Color32::from_rgb(124, 58, 237),  // DEBUG
        Color32::from_rgb(21, 128, 61),   // GAME
        Color32::from_rgb(147, 51, 234),  // EMOTE
        Color32::from_rgb(15, 118, 110),  // ACCESS
        Color32::from_rgb(79, 70, 229),   // WHISPER
        Color32::from_rgb(180, 83, 9),    // ADMINPRIVATE
        Color32::from_rgb(3, 105, 161),   // LOOC
        Color32::from_rgb(185, 28, 28),   // ADMIN
        Color32::from_rgb(194, 65, 12),   // PDA
        Color32::from_rgb(190, 24, 93),   // OOC
        Color32::from_rgb(220, 38, 38),   // TRAITOR
        Color32::from_rgb(161, 98, 7),    // MENTORSAY
        Color32::from_rgb(107, 114, 128), // DEADCHAT
        Color32::from_rgb(148, 163, 184), // OTHER
    ],
};

static DARK: Theme = Theme {
    dark: true,
    panel: Color32::from_rgb(21, 23, 28),
    top_bar: Color32::from_rgb(28, 31, 38),
    header: Color32::from_rgb(35, 40, 51),
    stripe: Color32::from_rgb(32, 36, 45),
    text: Color32::from_rgb(231, 233, 238),
    weak_text: Color32::from_rgb(149, 157, 172),
    accent: Color32::from_rgb(122, 162, 247),
    accent_text: Color32::from_rgb(13, 17, 23),
    danger: Color32::from_rgb(255, 123, 114),
    palette: [
        Color32::from_rgb(154, 164, 178), // MISC
        Color32::from_rgb(86, 199, 238),  // SAY
        Color32::from_rgb(255, 107, 107), // ATTACK
        Color32::from_rgb(167, 139, 250), // DEBUG
        Color32::from_rgb(125, 216, 125), // GAME
        Color32::from_rgb(211, 164, 255), // EMOTE
        Color32::from_rgb(79, 209, 197),  // ACCESS
        Color32::from_rgb(139, 156, 255), // WHISPER
        Color32::from_rgb(246, 193, 119), // ADMINPRIVATE
        Color32::from_rgb(125, 211, 252), // LOOC
        Color32::from_rgb(255, 123, 114), // ADMIN
        Color32::from_rgb(255, 184, 108), // PDA
        Color32::from_rgb(255, 138, 199), // OOC
        Color32::from_rgb(255, 107, 107), // TRAITOR
        Color32::from_rgb(229, 192, 123), // MENTORSAY
        Color32::from_rgb(124, 135, 152), // DEADCHAT
        Color32::from_rgb(107, 114, 128), // OTHER
    ],
};
