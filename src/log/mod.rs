mod admin;
mod badges;
mod columns;
mod dedup;
mod keys;
mod marks;
mod parse;
mod sanitize;

pub(crate) use admin::*;
pub(crate) use columns::*;
pub(crate) use dedup::*;
pub(crate) use parse::*;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;

/// All log levels/categories known to the parser
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Misc,
    Say,
    Attack,
    Debug,
    Game,
    Emote,
    Access,
    Whisper,
    AdminPrivate,
    Looc,
    Admin,
    Pda,
    Ooc,
    Traitor,
    MentorSay,
    DeadChat,
    Other,
}

impl Category {
    pub const COUNT: usize = 17;

    pub const ALL: [Self; Self::COUNT] = [
        Self::Misc,
        Self::Say,
        Self::Attack,
        Self::Debug,
        Self::Game,
        Self::Emote,
        Self::Access,
        Self::Whisper,
        Self::AdminPrivate,
        Self::Looc,
        Self::Admin,
        Self::Pda,
        Self::Ooc,
        Self::Traitor,
        Self::MentorSay,
        Self::DeadChat,
        Self::Other,
    ];

    pub fn from_tag(tag: &str) -> Self {
        match tag {
            "MISC" => Self::Misc,
            "SAY" => Self::Say,
            "ATTACK" => Self::Attack,
            "DEBUG" => Self::Debug,
            "GAME" => Self::Game,
            "EMOTE" => Self::Emote,
            "ACCESS" => Self::Access,
            "WHISPER" => Self::Whisper,
            "ADMINPRIVATE" => Self::AdminPrivate,
            "LOOC" => Self::Looc,
            "ADMIN" => Self::Admin,
            "PDA" => Self::Pda,
            "OOC" => Self::Ooc,
            "TRAITOR" => Self::Traitor,
            "MENTORSAY" => Self::MentorSay,
            "DEADCHAT" => Self::DeadChat,
            _ => Self::Other,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Misc => "MISC",
            Self::Say => "SAY",
            Self::Attack => "ATTACK",
            Self::Debug => "DEBUG",
            Self::Game => "GAME",
            Self::Emote => "EMOTE",
            Self::Access => "ACCESS",
            Self::Whisper => "WHISPER",
            Self::AdminPrivate => "ADMINPRIVATE",
            Self::Looc => "LOOC",
            Self::Admin => "ADMIN",
            Self::Pda => "PDA",
            Self::Ooc => "OOC",
            Self::Traitor => "TRAITOR",
            Self::MentorSay => "MENTORSAY",
            Self::DeadChat => "DEADCHAT",
            Self::Other => "OTHER",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Misc => 0,
            Self::Say => 1,
            Self::Attack => 2,
            Self::Debug => 3,
            Self::Game => 4,
            Self::Emote => 5,
            Self::Access => 6,
            Self::Whisper => 7,
            Self::AdminPrivate => 8,
            Self::Looc => 9,
            Self::Admin => 10,
            Self::Pda => 11,
            Self::Ooc => 12,
            Self::Traitor => 13,
            Self::MentorSay => 14,
            Self::DeadChat => 15,
            Self::Other => 16,
        }
    }
}

/// Timestamp in the log header (`YYYY-MM-DD HH:MM:SS.mmm`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
}

impl LogTime {
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 23
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes[10] != b' '
            || bytes[13] != b':'
            || bytes[16] != b':'
            || bytes[19] != b'.'
        {
            return None;
        }

        Some(Self {
            year: parse_digits(&bytes[0..4])?,
            month: parse_digits(&bytes[5..7])? as u8,
            day: parse_digits(&bytes[8..10])? as u8,
            hour: parse_digits(&bytes[11..13])? as u8,
            minute: parse_digits(&bytes[14..16])? as u8,
            second: parse_digits(&bytes[17..19])? as u8,
            millis: parse_digits(&bytes[20..23])?,
        })
    }

    pub fn date_string(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl fmt::Display for LogTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}.{:03}",
            self.hour, self.minute, self.second, self.millis
        )
    }
}

fn parse_digits(bytes: &[u8]) -> Option<u16> {
    let mut value = 0_u16;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u16::from(byte - b'0');
    }
    Some(value)
}

/// An interned player identity. `key` keeps the most human-readable variant seen in the log, while `ckey` is the canonical form
#[derive(Clone, Debug)]
pub struct PlayerKey {
    pub key: String,
    pub ckey: String,
    pub system: bool,
}

impl PlayerKey {
    pub fn matches_name(&self, name: &str) -> bool {
        normalize_identity(&self.ckey) == normalize_identity(name)
    }
}

fn normalize_identity(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// An inline `key/(Character Name)` occurrence in a message
#[derive(Clone, Debug)]
pub struct TextBadge {
    pub key: u32,
    pub key_start: usize,
    pub key_end: usize,
    pub name: Option<Range<usize>>,
}

/// Non-player inline decorations: mob refs and locations with coordinates
#[derive(Clone, Debug)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    pub kind: MarkKind,
}

#[derive(Clone, Debug)]
pub enum MarkKind {
    Mob,
    /// `(Location Name (x,y,z))`; ranges point inside the outer parentheses
    Location {
        name: Range<usize>,
        coords: Range<usize>,
    },
    /// `ckey (as Character Name at x,y,z)` or `ckey (usr:Name[x,y,z])`
    AsCharacter {
        key: u32,
        key_end: usize,
        name: Range<usize>,
        coords: Range<usize>,
    },
    /// A known character name mentioned without a key (e.g. in a salary line)
    Character {
        key: u32,
        name: Range<usize>,
    },
    /// Radio frequency `[135.9]`
    Frequency {
        value: Range<usize>,
        spans: Option<Range<usize>>,
    },
    /// `PM: sender → recipient:` or `Ticket #N: sender → recipient:`
    AdminPm {
        ticket: Option<u32>,
        sender: u32,
        recipient: u32,
    },
    /// A standalone ticket reference
    TicketRef {
        number: u32,
    },
    /// IP address / CID token hidden by "Hide sensitive"
    Sensitive {
        value: Range<usize>,
    },
}

/// A character name bound to the player key that played it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub name: String,
    pub key: u32,
}

/// One physical line of the log
#[derive(Clone, Debug)]
pub struct LogRow {
    /// 1-based physical line number in the source file
    pub line_no: u32,
    pub time: Option<LogTime>,
    pub category: Category,
    pub message: String,
    /// `true` for ` - ` detail lines that belong to the previous entry
    pub continuation: bool,
    /// Runtime-error block: never reformat these lines
    pub runtime: bool,
    /// Badges over [`Self::message`] (original formatting)
    pub badges: Vec<TextBadge>,
    /// Mob/location decorations over [`Self::message`]
    pub marks: Vec<Mark>,
    /// Cleaned-up variant used when the "Format" option is enabled
    pub formatted: Option<FormattedMessage>,
}

/// A message with HTML tags/entities removed, whitespace collapsed and badges located in the cleaned text
#[derive(Clone, Debug)]
pub struct FormattedMessage {
    pub text: String,
    pub badges: Vec<TextBadge>,
    pub marks: Vec<Mark>,
    pub admin_ticket: Option<u32>,
}

/// A fully parsed log file, flattened into one row per physical line
#[derive(Clone, Debug, Default)]
pub struct LogFile {
    pub rows: Vec<LogRow>,
    /// Unique players in order of first appearance
    pub keys: Vec<PlayerKey>,
    /// Number of top-level entries (rows that are not continuations)
    pub entry_count: usize,
    pub first_time: Option<LogTime>,
    pub last_time: Option<LogTime>,
    /// Character-name ↔ player bindings collected from the log
    pub characters: Vec<Character>,
    /// Rows shown when Dedup is enabled (consecutive tgui duplicates removed)
    pub dedup_indices: Vec<u32>,
    /// Character widths for the virtual columns in Format mode
    pub format_columns: FormatColumns,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FormatColumns {
    pub key_chars: usize,
    pub name_chars: usize,
    pub admin_ticket_chars: usize,
    pub admin_sender_chars: usize,
    pub admin_recipient_chars: usize,
}

impl LogFile {
    pub fn parse(text: &str) -> Self {
        let mut rows: Vec<LogRow> = Vec::with_capacity(text.len() / 90);
        let mut keys: Vec<PlayerKey> = Vec::new();
        let mut key_ids: HashMap<String, u32> = HashMap::new();
        let mut entry_count = 0;
        let mut first_time = None;
        let mut last_time = None;
        let mut runtime_block = false;
        for (index, line) in text.lines().enumerate() {
            let line_no = index as u32 + 1;

            let row = if let Some(detail) = line.strip_prefix(" - ") {
                let category = rows
                    .last()
                    .map_or(Category::Other, |row: &LogRow| row.category);
                make_row(
                    line_no,
                    None,
                    category,
                    detail,
                    true,
                    runtime_block,
                    &mut keys,
                    &mut key_ids,
                )
            } else if let Some((time, rest)) = split_header(line) {
                entry_count += 1;
                first_time.get_or_insert(time);
                last_time = Some(time);

                let (category, message) = split_category(rest);
                runtime_block = message.starts_with("runtime error:")
                    || message.contains("overridden. Use override = TRUE");
                make_row(
                    line_no,
                    Some(time),
                    category,
                    message,
                    false,
                    runtime_block,
                    &mut keys,
                    &mut key_ids,
                )
            } else {
                make_row(
                    line_no,
                    None,
                    Category::Other,
                    line,
                    true,
                    runtime_block,
                    &mut keys,
                    &mut key_ids,
                )
            };

            rows.push(row);
        }

        let characters = collect_characters(&rows, &keys);
        let name_map: HashMap<String, u32> = characters
            .iter()
            .map(|character| (character.name.clone(), character.key))
            .collect();
        add_currency_marks(&mut rows, &name_map);

        let dedup_indices = compute_tgui_dedup(&rows);
        let format_columns = compute_format_columns(&rows, &keys);

        Self {
            rows,
            keys,
            entry_count,
            first_time,
            last_time,
            characters,
            dedup_indices,
            format_columns,
        }
    }

    pub fn time_range(&self) -> Option<(LogTime, LogTime)> {
        Some((self.first_time?, self.last_time?))
    }
}
