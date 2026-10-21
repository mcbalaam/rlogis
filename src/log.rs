//! Parsing of Space Station 13 `game.log` files.
//!
//! Each physical line becomes one [`LogRow`], so the table mirrors the file
//! one-to-one. Lines that continue a previous entry (the ` - ` prefixed
//! details of stack traces, actions, etc.) are kept as separate rows and
//! flagged with [`LogRow::continuation`].
//!
//! Player keys are interned into [`LogFile::keys`]. A log token like
//! `Fenix Armory/(Fenix Armory)` gives a display key (`Fenix Armory`) and a
//! canonical ckey (`fenixarmory`); both variants are bound to the same entry,
//! so badges and colors stay stable across the file.

use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

/// All log levels/categories known to the parser.
///
/// Unknown tags are mapped to [`Category::Other`] so a malformed or newer log
/// never breaks parsing.
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

/// Timestamp in the log header (`YYYY-MM-DD HH:MM:SS.mmm`).
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

/// An interned player identity. `key` keeps the most human-readable variant
/// seen in the log, while `ckey` is the canonical, lowercase form.
#[derive(Clone, Debug)]
pub struct PlayerKey {
    pub key: String,
    pub ckey: String,
    /// `true` for the `*no key*` pseudo-player.
    pub system: bool,
}

impl PlayerKey {
    /// True when `name` is just another spelling of this key (for example
    /// `maleK1337` / `Malek1337` or `Fenix Armory` / `fenixarmory`).
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

/// An inline `key/(Character Name)` occurrence in a message.
#[derive(Clone, Debug)]
pub struct TextBadge {
    pub key: u32,
    pub key_start: usize,
    pub key_end: usize,
    /// Byte range of the character name inside the parentheses.
    pub name: Option<Range<usize>>,
}

/// Non-player inline decorations: mob refs and locations with coordinates.
#[derive(Clone, Debug)]
pub struct Mark {
    pub start: usize,
    pub end: usize,
    pub kind: MarkKind,
}

#[derive(Clone, Debug)]
pub enum MarkKind {
    Mob,
    /// `(Location Name (x,y,z))`; ranges point inside the outer parentheses.
    Location {
        name: Range<usize>,
        coords: Range<usize>,
    },
}

/// One physical line of the log.
#[derive(Clone, Debug)]
pub struct LogRow {
    /// 1-based physical line number in the source file.
    pub line_no: u32,
    pub time: Option<LogTime>,
    pub category: Category,
    pub message: String,
    /// `true` for ` - ` detail lines that belong to the previous entry.
    pub continuation: bool,
    /// Badges over [`Self::message`] (original formatting).
    pub badges: Vec<TextBadge>,
    /// Mob/location decorations over [`Self::message`].
    pub marks: Vec<Mark>,
    /// Cleaned-up variant used when the "Format" option is enabled.
    pub formatted: Option<FormattedMessage>,
}

/// A message with HTML tags/entities removed, whitespace collapsed and badges
/// located in the cleaned text.
#[derive(Clone, Debug)]
pub struct FormattedMessage {
    pub text: String,
    pub badges: Vec<TextBadge>,
    pub marks: Vec<Mark>,
}

/// A fully parsed log file, flattened into one row per physical line.
#[derive(Clone, Debug, Default)]
pub struct LogFile {
    pub rows: Vec<LogRow>,
    /// Unique players in order of first appearance.
    pub keys: Vec<PlayerKey>,
    /// Number of top-level entries (rows that are not continuations).
    pub entry_count: usize,
    pub first_time: Option<LogTime>,
    pub last_time: Option<LogTime>,
}

impl LogFile {
    pub fn parse(text: &str) -> Self {
        let mut rows: Vec<LogRow> = Vec::with_capacity(text.len() / 90);
        let mut keys: Vec<PlayerKey> = Vec::new();
        let mut key_ids: HashMap<String, u32> = HashMap::new();
        let mut entry_count = 0;
        let mut first_time = None;
        let mut last_time = None;

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
                    &mut keys,
                    &mut key_ids,
                )
            } else if let Some((time, rest)) = split_header(line) {
                entry_count += 1;
                first_time.get_or_insert(time);
                last_time = Some(time);

                let (category, message) = split_category(rest);
                make_row(
                    line_no,
                    Some(time),
                    category,
                    message,
                    false,
                    &mut keys,
                    &mut key_ids,
                )
            } else {
                // Not a line we understand; keep it visible instead of
                // dropping data.
                make_row(
                    line_no,
                    None,
                    Category::Other,
                    line,
                    true,
                    &mut keys,
                    &mut key_ids,
                )
            };

            rows.push(row);
        }

        Self {
            rows,
            keys,
            entry_count,
            first_time,
            last_time,
        }
    }

    pub fn time_range(&self) -> Option<(LogTime, LogTime)> {
        Some((self.first_time?, self.last_time?))
    }
}

fn make_row(
    line_no: u32,
    time: Option<LogTime>,
    category: Category,
    message: &str,
    continuation: bool,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> LogRow {
    let badges = parse_badges(message, keys, key_ids);
    let marks = parse_marks(message);

    let cleaned = clean_message(message);
    let formatted = (cleaned != message).then(|| FormattedMessage {
        badges: parse_badges(&cleaned, keys, key_ids),
        marks: parse_marks(&cleaned),
        text: cleaned,
    });

    LogRow {
        line_no,
        time,
        category,
        message: message.to_owned(),
        continuation,
        badges,
        marks,
        formatted,
    }
}

/// Strips HTML tags, decodes entities and collapses whitespace.
fn clean_message(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut index = 0;

    while index < message.len() {
        let rest = &message[index..];
        let character = rest.chars().next().expect("non-empty slice");

        if character == '<'
            && let Some(end) = rest.find('>')
        {
            let tag = &rest[..=end];

            // BYOND PM links carry the real ckey in `priv_msg=`. Use it
            // instead of the anchor text, which may be spelled differently.
            if let Some(key) = extract_priv_msg(tag) {
                let after_tag = &rest[end + 1..];
                if let Some(close) = find_closing_anchor(after_tag) {
                    for character in key.chars() {
                        push_normalized(&mut out, character);
                    }
                    index += end + 1 + close + "</a>".len();
                    continue;
                }
            }

            index += end + 1;
            continue;
        }

        if character == '&'
            && let Some((decoded, consumed)) = decode_entity(rest)
        {
            push_normalized(&mut out, decoded);
            index += consumed;
            continue;
        }

        push_normalized(&mut out, character);
        index += character.len_utf8();
    }

    while out.ends_with(' ') {
        out.pop();
    }
    out
}

fn push_normalized(out: &mut String, character: char) {
    if character.is_whitespace() {
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    } else {
        out.push(character);
    }
}

fn extract_priv_msg(tag: &str) -> Option<&str> {
    const MARKER: &str = "priv_msg=";
    let start = tag.find(MARKER)? + MARKER.len();
    let rest = &tag[start..];
    let quote = rest.chars().next()?;

    if quote == '\'' || quote == '"' {
        let value = &rest[1..];
        let end = value.find(quote)?;
        Some(&value[..end])
    } else {
        let end = rest
            .find(|character: char| {
                character == '&' || character == '\'' || character == '"' || character == '>'
            })
            .unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

fn find_closing_anchor(text: &str) -> Option<usize> {
    text.as_bytes()
        .windows(4)
        .position(|window| window.eq_ignore_ascii_case(b"</a>"))
}

fn decode_entity(text: &str) -> Option<(char, usize)> {
    let end = text.find(';')?;
    if end > 12 {
        return None;
    }

    let body = &text[1..end];
    let decoded = if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
    } else if let Some(decimal) = body.strip_prefix('#') {
        char::from_u32(decimal.parse().ok()?)?
    } else {
        match body {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => ' ',
            _ => return None,
        }
    };

    Some((decoded, end + 1))
}

/// Finds every `key/(Character Name)` match in a message and interns the key.
fn parse_badges(
    message: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> Vec<TextBadge> {
    let mut badges = Vec::new();
    let mut search = 0;
    let mut first_match = true;

    while let Some(offset) = message[search..].find("/(") {
        let slash = search + offset;
        let name_open = slash + 2;

        let (key_start, key_end) = key_range(message, slash, first_match);
        first_match = false;

        if key_start == key_end {
            search = name_open;
            continue;
        }

        let key_text = &message[key_start..key_end];
        if key_text.is_empty() {
            search = name_open;
            continue;
        }

        let key_id = intern_key(key_text, keys, key_ids);

        if let Some(close_offset) = message[name_open..].find(')') {
            let name_end = name_open + close_offset;
            let name = (name_end > name_open).then_some(name_open..name_end);
            badges.push(TextBadge {
                key: key_id,
                key_start,
                key_end,
                name,
            });
            search = name_end + 1;
        } else {
            // No closing parenthesis: badge the key and leave the rest intact.
            badges.push(TextBadge {
                key: key_id,
                key_start,
                key_end,
                name: None,
            });
            search = name_open;
        }
    }

    badges
}

/// Returns the byte range of the player key preceding the `/(`.
///
/// Usually the key is a single token. Some servers log the human-readable key
/// (`Fenix Armory`) at the start of the message; in that case the whole prefix
/// is part of the key.
fn key_range(message: &str, slash: usize, first_match: bool) -> (usize, usize) {
    // The `*no key*` pseudo-player keeps its asterisks so the badge mirrors
    // the original log formatting.
    const NO_KEY: &str = "*no key*";
    if message[..slash].ends_with(NO_KEY) {
        return (slash - NO_KEY.len(), slash);
    }

    if first_match {
        let prefix = &message[..slash];
        let trimmed = prefix.trim();
        let start = prefix.len() - prefix.trim_start().len();
        if !trimmed.is_empty()
            && trimmed.len() <= 24
            && trimmed.split_whitespace().count() <= 3
            && trimmed
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b' ')
        {
            return (start, slash);
        }
    }

    let bytes = message.as_bytes();
    let mut start = slash;
    while start > 0 && is_key_byte(bytes[start - 1]) {
        start -= 1;
    }

    // Don't badge text that belongs to an HTML tag such as `</a>`.
    if start > 0 && bytes[start - 1] == b'>' {
        return (slash, slash);
    }

    (start, slash)
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Finds mob references and `(Location (x,y,z))` decorations.
fn parse_marks(message: &str) -> Vec<Mark> {
    let mut marks = find_mobs(message);
    marks.extend(find_locations(message));
    marks.sort_by_key(|mark| mark.start);
    marks
}

fn find_mobs(message: &str) -> Vec<Mark> {
    let bytes = message.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0;

    while index + 4 <= bytes.len() {
        if bytes[index..].starts_with(b"mob_") {
            let start = index;
            let mut end = index + 4;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }

            let boundary_before = start == 0 || !is_key_byte(bytes[start - 1]);
            let boundary_after = end == bytes.len() || !is_key_byte(bytes[end]);
            if end > index + 4 && boundary_before && boundary_after {
                marks.push(Mark {
                    start,
                    end,
                    kind: MarkKind::Mob,
                });
                index = end;
                continue;
            }
        }
        index += 1;
    }

    marks
}

/// Matches `(Location Name (x,y,z))` and captures the location name and the
/// coordinates separately.
fn find_locations(message: &str) -> Vec<Mark> {
    let bytes = message.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'('
            && let Some((coords_end, coords)) = parse_coords(message, index)
        {
            let after_coords = &message[coords_end..];
            let trimmed = after_coords.trim_start();
            if trimmed.starts_with(')') {
                let outer_close = coords_end + (after_coords.len() - trimmed.len());

                if let Some(name_open) = message[..index].rfind('(') {
                    let name_raw = &message[name_open + 1..index];
                    let name_start = name_open + 1;

                    // Trim whitespace around the location name.
                    let leading = name_raw.len() - name_raw.trim_start().len();
                    let trailing = name_raw.trim_end().len();
                    let name = name_start + leading..name_start + trailing;

                    if name.start < name.end && is_location_name(&message[name.clone()]) {
                        marks.push(Mark {
                            start: name_open,
                            end: outer_close + 1,
                            kind: MarkKind::Location { name, coords },
                        });
                        index = outer_close + 1;
                        continue;
                    }
                }
            }
        }
        index += 1;
    }

    marks
}

fn parse_coords(message: &str, open: usize) -> Option<(usize, Range<usize>)> {
    let bytes = message.as_bytes();
    let mut index = open + 1;

    for axis in 0..3 {
        if index < bytes.len() && bytes[index] == b'-' {
            index += 1;
        }
        let digits = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == digits {
            return None;
        }
        if axis < 2 {
            if index >= bytes.len() || bytes[index] != b',' {
                return None;
            }
            index += 1;
        }
    }

    if index < bytes.len() && bytes[index] == b')' {
        Some((index + 1, open + 1..index))
    } else {
        None
    }
}

fn is_location_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name.chars().any(char::is_alphanumeric)
        && !name.chars().any(|character| "()[]{}<>".contains(character))
}

fn intern_key(text: &str, keys: &mut Vec<PlayerKey>, key_ids: &mut HashMap<String, u32>) -> u32 {
    let system = text.eq_ignore_ascii_case("*no key*");
    let ckey = if system {
        "*no key*".to_owned()
    } else {
        canonical_ckey(text)
    };

    if let Some(&id) = key_ids.get(&ckey) {
        let existing = &mut keys[id as usize];
        if key_quality(text) > key_quality(&existing.key) {
            existing.key = text.to_owned();
        }
        return id;
    }

    let id = keys.len() as u32;
    key_ids.insert(ckey.clone(), id);
    keys.push(PlayerKey {
        key: text.to_owned(),
        ckey,
        system,
    });
    id
}

/// BYOND ckeys are lowercase and have no spaces: `Vladislav Vorobyov` becomes
/// `vladislavvorobyov`.
fn canonical_ckey(key: &str) -> String {
    key.chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Prefer the variant that looks most like a human-readable key.
fn key_quality(key: &str) -> (usize, usize) {
    let spaces = key.bytes().filter(|byte| *byte == b' ').count();
    let uppercase = key
        .chars()
        .filter(|character| character.is_uppercase())
        .count();
    (spaces, uppercase)
}

/// Splits `[2026-10-17 17:23:55.085] MISC: text` into time and `MISC: text`.
fn split_header(line: &str) -> Option<(LogTime, &str)> {
    if !line.starts_with('[') {
        return None;
    }
    let header_end = line.find("] ")?;
    let time = LogTime::parse(line.get(1..header_end)?)?;
    Some((time, line.get(header_end + 2..)?))
}

/// Splits `MISC: text` into a category and the message.
fn split_category(rest: &str) -> (Category, &str) {
    if let Some((tag, message)) = rest.split_once(": ")
        && !tag.is_empty()
        && tag.len() <= 16
        && tag.bytes().all(|byte| byte.is_ascii_uppercase())
    {
        return (Category::from_tag(tag), message);
    }
    (Category::Other, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_continuations() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:23:55.085] DEBUG: World loaded!\n",
            "[2026-10-17 17:24:02.500] SAY: user/(Name) (mob_1) \"Hello\" (bar (10,20,2))\n",
            " - Using: /datum/orbit_menu [0x2100161c]\n",
            " - Action: orbit {\"ref\":\"[mob_3356]\"}\n",
            "[2026-10-17 17:24:03.000] Starting up round ID 9452.\n",
        ));

        assert_eq!(log.rows.len(), 5);
        assert_eq!(log.entry_count, 3);
        assert_eq!(log.rows[0].line_no, 1);
        assert_eq!(log.rows[0].time.unwrap().to_string(), "17:23:55.085");
        assert_eq!(log.rows[0].category, Category::Debug);
        assert_eq!(log.rows[0].message, "World loaded!");

        assert_eq!(log.rows[1].category, Category::Say);
        assert_eq!(log.rows[1].time.unwrap().hour, 17);
        assert_eq!(log.rows[1].badges.len(), 1);
        assert_eq!(log.keys[0].key, "user");
        assert_eq!(log.keys[0].ckey, "user");

        assert!(log.rows[2].continuation);
        assert_eq!(log.rows[2].category, Category::Say);
        assert_eq!(log.rows[2].message, "Using: /datum/orbit_menu [0x2100161c]");
        assert_eq!(log.rows[2].line_no, 3);

        assert!(log.rows[4].time.is_some());
        assert_eq!(log.rows[4].category, Category::Other);
        assert_eq!(log.rows[4].message, "Starting up round ID 9452.");
    }

    #[test]
    fn binds_key_and_ckey() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:24:02.500] OOC: Fenix Armory/(Fenix Armory) \"hi\"\n",
            "[2026-10-17 17:24:03.500] SAY: fenixarmory/(Fenix Armory) \"hello\"\n",
        ));

        assert_eq!(log.keys.len(), 1);
        assert_eq!(log.keys[0].key, "Fenix Armory");
        assert_eq!(log.keys[0].ckey, "fenixarmory");
        assert_eq!(log.rows[0].badges[0].key, 0);
        assert_eq!(log.rows[1].badges[0].key, 0);

        let name = log.rows[0].badges[0].name.clone().unwrap();
        assert_eq!(&log.rows[0].message[name], "Fenix Armory");
    }

    #[test]
    fn extracts_multiple_badges_and_keeps_no_key() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:24:02.500] ATTACK: Karavill/(Name) attacked victim_1/(Victim)\n",
            "[2026-10-17 17:24:03.500] SAY: *no key*/(Poly) \"Squawk\"\n",
        ));

        let keys: Vec<&str> = log.keys.iter().map(|player| player.ckey.as_str()).collect();
        assert_eq!(keys, ["karavill", "victim_1", "*no key*"]);
        assert_eq!(log.rows[0].badges.len(), 2);
        assert_eq!(log.rows[0].badges[0].key, 0);
        assert_eq!(log.rows[0].badges[1].key, 1);

        let pseudo_badge = &log.rows[1].badges[0];
        let pseudo = &log.keys[pseudo_badge.key as usize];
        assert!(pseudo.system);
        assert_eq!(pseudo.ckey, "*no key*");
        assert_eq!(
            &log.rows[1].message[pseudo_badge.key_start..pseudo_badge.key_end],
            "*no key*"
        );

        let second_name = log.rows[0].badges[1].name.clone().unwrap();
        assert_eq!(&log.rows[0].message[second_name], "Victim");
    }

    #[test]
    fn formats_html_player_tags() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) начал отвечать на тикет #4.\n",
            "[2026-10-17 17:33:28.000] SAY: Fenix Armory/(Vladislav Vorobyov) said &#34;hi&#34; &amp; left.\n",
        ));

        let formatted = log.rows[0].formatted.as_ref().unwrap();
        assert_eq!(
            formatted.text,
            "mrsnake/(Магуро Хамати) начал отвечать на тикет #4."
        );
        assert_eq!(formatted.badges.len(), 1);
        let player = &log.keys[formatted.badges[0].key as usize];
        assert_eq!(player.ckey, "mrsnake");
        let name = formatted.badges[0].name.clone().unwrap();
        assert_eq!(&formatted.text[name], "Магуро Хамати");

        let formatted = log.rows[1].formatted.as_ref().unwrap();
        assert_eq!(
            formatted.text,
            "Fenix Armory/(Vladislav Vorobyov) said \"hi\" & left."
        );
        assert_eq!(
            log.keys[formatted.badges[0].key as usize].ckey,
            "fenixarmory"
        );
    }

    #[test]
    fn uses_priv_msg_from_html_for_ckey() {
        let log = LogFile::parse(
            "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=stupid1'>Stupid_1</a>/(Stupid_1) started.\n",
        );

        let formatted = log.rows[0].formatted.as_ref().unwrap();
        assert_eq!(formatted.text, "stupid1/(Stupid_1) started.");
        let player = &log.keys[formatted.badges[0].key as usize];
        assert_eq!(player.ckey, "stupid1");
        assert!(player.matches_name("Stupid_1"));
    }

    #[test]
    fn raw_badges_skip_html_tag_text() {
        let log = LogFile::parse(
            "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) started.\n",
        );
        assert!(log.rows[0].badges.is_empty());
        assert!(!log.keys.iter().any(|player| player.ckey == "a"));
    }

    #[test]
    fn matches_key_with_duplicate_mob_names() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:25:26.451] OOC: maleK1337/(Malek1337) \"tr\"\n",
            "[2026-10-17 17:24:02.500] OOC: Fenix Armory/(Fenix Armory) \"hi\"\n",
            "[2026-10-17 17:24:03.500] SAY: mrsnake/(Магуро Хамати) \"hello\"\n",
        ));

        let male = &log.keys[log.rows[0].badges[0].key as usize];
        assert!(male.matches_name("Malek1337"));

        let fenix = &log.keys[log.rows[1].badges[0].key as usize];
        assert_eq!(fenix.ckey, "fenixarmory");
        assert!(fenix.matches_name("Fenix Armory"));

        let mrsnake = &log.keys[log.rows[2].badges[0].key as usize];
        assert!(!mrsnake.matches_name("Магуро Хамати"));
    }

    #[test]
    fn finds_mob_and_location_marks() {
        let log = LogFile::parse(
            "[2026-10-17 17:30:01.917] ATTACK: *no key*/(goldgrub) (mob_491) has eaten the plasma ore! (Laser Tag Arena (84,109,2))\n",
        );
        let row = &log.rows[0];

        let mob = row
            .marks
            .iter()
            .find(|mark| matches!(mark.kind, MarkKind::Mob))
            .unwrap();
        assert_eq!(&row.message[mob.start..mob.end], "mob_491");

        let (full, name, coords) = row
            .marks
            .iter()
            .find_map(|mark| match &mark.kind {
                MarkKind::Location { name, coords } => Some((mark, name.clone(), coords.clone())),
                MarkKind::Mob => None,
            })
            .unwrap();
        assert_eq!(
            &row.message[full.start..full.end],
            "(Laser Tag Arena (84,109,2))"
        );
        assert_eq!(&row.message[name], "Laser Tag Arena");
        assert_eq!(&row.message[coords], "84,109,2");
    }

    #[test]
    fn finds_marks_in_formatted_text() {
        let log = LogFile::parse(
            "[2026-10-17 17:33:27.631] ADMIN: <a href='byond://?priv_msg=mrsnake'>Mrsnake</a>/(Магуро Хамати) did (Laser Tag Arena (84,109,2))\n",
        );
        let formatted = log.rows[0].formatted.as_ref().unwrap();
        let (name, coords) = formatted
            .marks
            .iter()
            .find_map(|mark| match &mark.kind {
                MarkKind::Location { name, coords } => Some((name.clone(), coords.clone())),
                MarkKind::Mob => None,
            })
            .unwrap();
        assert_eq!(&formatted.text[name], "Laser Tag Arena");
        assert_eq!(&formatted.text[coords], "84,109,2");
    }

    #[test]
    fn handles_crlf_and_empty_input() {
        let log = LogFile::parse("[2026-10-17 17:23:55.085] GAME: one\r\n - detail\r\n");
        assert_eq!(log.rows.len(), 2);
        assert_eq!(log.rows[0].message, "one");
        assert_eq!(log.rows[1].message, "detail");

        assert!(LogFile::parse("").rows.is_empty());
    }

    #[test]
    fn unknown_category_is_preserved_as_message() {
        let log = LogFile::parse("[2026-10-17 17:23:55.085] WAT: something\n");
        assert_eq!(log.rows[0].category, Category::Other);
        assert_eq!(log.rows[0].message, "something");
    }

    #[test]
    fn time_range_is_tracked() {
        let log = LogFile::parse(
            "[2026-10-17 17:23:55.085] MISC: a\n[2026-10-17 18:23:55.999] MISC: b\n",
        );
        let (first, last) = log.time_range().unwrap();
        assert_eq!(first.to_string(), "17:23:55.085");
        assert_eq!(last.to_string(), "18:23:55.999");
    }
}
