use egui::Color32;

pub struct Channel {
    pub name: &'static str,
    /// Standard frequency, `""` when the channel has none
    pub frequency: &'static str,
    /// Chat span classes that identify this channel
    pub spans: &'static [&'static str],
    pub color: Color32,
}

/// Colour used for unknown channels and non-standard frequencies
pub const DEFAULT_COLOR: Color32 = Color32::from_rgb(0x20, 0xA0, 0x20);

const GRAY: Color32 = Color32::from_rgb(0x70, 0x80, 0x90);

static CHANNELS: &[Channel] = &[
    Channel {
        name: "Uplink",
        frequency: "121.1",
        spans: &["uplink"],
        color: Color32::from_rgb(0x80, 0x00, 0x80),
    },
    Channel {
        name: "Syndicate",
        frequency: "121.3",
        spans: &["syndicate", "encrypted"],
        color: Color32::from_rgb(0x8B, 0x00, 0x00),
    },
    Channel {
        name: "CTF-Red",
        frequency: "121.5",
        spans: &["ctf_red"],
        color: Color32::from_rgb(0xD2, 0x22, 0x2D),
    },
    Channel {
        name: "CTF-Blue",
        frequency: "121.7",
        spans: &["ctf_blue"],
        color: Color32::from_rgb(0x1E, 0x4B, 0xB8),
    },
    Channel {
        name: "CTF-Green",
        frequency: "121.9",
        spans: &["ctf_green"],
        color: Color32::from_rgb(0x00, 0xA5, 0x50),
    },
    Channel {
        name: "CTF-Yellow",
        frequency: "122.1",
        spans: &["ctf_yellow"],
        color: Color32::from_rgb(0xC9, 0xA2, 0x27),
    },
    Channel {
        name: "CentCom",
        frequency: "133.7",
        spans: &["centcom", "cent_com"],
        color: Color32::from_rgb(0x2F, 0x7F, 0xBF),
    },
    Channel {
        name: "Supply",
        frequency: "134.7",
        spans: &["supply", "cargo", "quartermaster"],
        color: Color32::from_rgb(0xB8, 0x86, 0x0B),
    },
    Channel {
        name: "Service",
        frequency: "134.9",
        spans: &["service"],
        color: Color32::from_rgb(0x9A, 0xCD, 0x32),
    },
    Channel {
        name: "Science",
        frequency: "135.1",
        spans: &["science", "research", "rd"],
        color: Color32::from_rgb(0xBA, 0x55, 0xD3),
    },
    Channel {
        name: "Command",
        frequency: "135.3",
        spans: &["command", "command_headset"],
        color: Color32::from_rgb(0xC8, 0xA0, 0x00),
    },
    Channel {
        name: "Medical",
        frequency: "135.5",
        spans: &["medical", "medical_headset"],
        color: Color32::from_rgb(0x56, 0xB4, 0xE9),
    },
    Channel {
        name: "Engineering",
        frequency: "135.7",
        spans: &["engineering", "engineering_headset"],
        color: Color32::from_rgb(0xD3, 0x5F, 0x00),
    },
    Channel {
        name: "Security",
        frequency: "135.9",
        spans: &["security", "security_headset"],
        color: Color32::from_rgb(0xCC, 0x00, 0x00),
    },
    Channel {
        name: "Entertainment",
        frequency: "141.5",
        spans: &["entertainment"],
        color: Color32::from_rgb(0x3C, 0xB3, 0x71),
    },
    Channel {
        name: "Hologrid",
        frequency: "143.3",
        spans: &["hologrid"],
        color: GRAY,
    },
    Channel {
        name: "Status Displays",
        frequency: "143.5",
        spans: &["status_displays"],
        color: GRAY,
    },
    Channel {
        name: "AI Private",
        frequency: "144.7",
        spans: &["ai_private", "ai"],
        color: Color32::from_rgb(0xE0, 0x52, 0x9E),
    },
    Channel {
        name: "Electropacks",
        frequency: "144.9",
        spans: &["electropacks"],
        color: GRAY,
    },
    Channel {
        name: "Locator Implants",
        frequency: "145.1",
        spans: &["implants", "locator"],
        color: GRAY,
    },
    Channel {
        name: "Radio Navigation Beacons",
        frequency: "145.5",
        spans: &["navigation", "beacons"],
        color: GRAY,
    },
    Channel {
        name: "Signallers",
        frequency: "145.7",
        spans: &["signallers", "signaller"],
        color: GRAY,
    },
    Channel {
        name: "Common",
        frequency: "145.9",
        spans: &["common", "radio"],
        color: DEFAULT_COLOR,
    },
    Channel {
        name: "Alien Hivemind",
        frequency: "",
        spans: &["alien", "hivemind"],
        color: Color32::from_rgb(0x55, 0x6B, 0x2F),
    },
    Channel {
        name: "Binary",
        frequency: "",
        spans: &["binary", "robot"],
        color: Color32::from_rgb(0x6C, 0x75, 0x7D),
    },
    Channel {
        name: "Changeling",
        frequency: "",
        spans: &["changeling"],
        color: Color32::from_rgb(0x46, 0x82, 0xB4),
    },
    Channel {
        name: "Cybersun",
        frequency: "",
        spans: &["cybersun"],
        color: Color32::from_rgb(0x00, 0xCE, 0xD1),
    },
    Channel {
        name: "Guild",
        frequency: "",
        spans: &["guild"],
        color: Color32::from_rgb(0x41, 0x69, 0xE1),
    },
    Channel {
        name: "Interdyne",
        frequency: "",
        spans: &["interdyne"],
        color: Color32::from_rgb(0x7B, 0x3F, 0x00),
    },
    Channel {
        name: "Magnets",
        frequency: "144.9",
        spans: &["magnets", "magnet"],
        color: GRAY,
    },
    Channel {
        name: "SolFed",
        frequency: "",
        spans: &["solfed"],
        color: Color32::from_rgb(0x19, 0x19, 0x70),
    },
    Channel {
        name: "Tarkon",
        frequency: "",
        spans: &["tarkon"],
        color: Color32::from_rgb(0x99, 0x65, 0x15),
    },
];

pub fn channel_for(spans: Option<&str>, frequency: &str) -> Option<&'static Channel> {
    if let Some(spans) = spans {
        for span in spans.split_whitespace() {
            let span = span
                .trim_matches(|character: char| !character.is_alphanumeric() && character != '_')
                .to_ascii_lowercase();
            if let Some(channel) = CHANNELS.iter().find(|c| c.spans.contains(&span.as_str())) {
                return Some(channel);
            }
        }
    }

    CHANNELS.iter().find(|c| c.frequency == frequency)
}

pub fn color_for(spans: Option<&str>, frequency: &str) -> Color32 {
    channel_for(spans, frequency).map_or(DEFAULT_COLOR, |channel| channel.color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_name_wins_over_frequency() {
        let command = color_for(Some("(spans:  command_headset )"), "135.9");
        assert_eq!(
            command,
            channel_for(Some("command_headset"), "").unwrap().color
        );
        assert_ne!(command, color_for(None, "135.9"));
    }

    #[test]
    fn known_frequency_maps_to_its_channel() {
        assert_eq!(
            color_for(None, "145.9"),
            color_for(Some("(spans:  )"), "145.9")
        );
        assert_eq!(channel_for(None, "135.3").unwrap().name, "Command");
    }

    #[test]
    fn unknown_is_green() {
        assert_eq!(color_for(None, "136.1"), DEFAULT_COLOR);
        assert_eq!(color_for(Some("(spans:  yell )"), "142.3"), DEFAULT_COLOR);
    }

    #[test]
    fn span_modifiers_are_skipped() {
        assert_eq!(
            color_for(Some("(spans:  command_headset yell )"), "145.9"),
            color_for(Some("(spans: command_headset)"), "145.9")
        );
    }
}
