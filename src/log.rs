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

use std::collections::{HashMap, HashSet};
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
    /// `ckey (as Character Name at x,y,z)` or `ckey (usr:Name[x,y,z])`.
    AsCharacter {
        key: u32,
        key_end: usize,
        name: Range<usize>,
        coords: Range<usize>,
    },
    /// A known character name mentioned without a key (e.g. in a salary line).
    Character {
        key: u32,
        name: Range<usize>,
    },
    /// Radio frequency `[135.9]` with the optional ` (spans: …)` tail.
    Frequency {
        /// Digits between the square brackets.
        value: Range<usize>,
        /// The whole ` (spans: …)` block, when the line carried one.
        spans: Option<Range<usize>>,
    },
    /// `PM: sender → recipient:` or `Ticket #N: sender → recipient:`.
    AdminPm {
        ticket: Option<u32>,
        sender: u32,
        recipient: u32,
    },
    /// A standalone ticket reference such as `Тикет #2 был закрыт`.
    TicketRef {
        number: u32,
    },
    /// IP address / CID token hidden by "Hide sensitive".
    Sensitive {
        value: Range<usize>,
    },
}

/// A character name bound to the player key that played it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub name: String,
    pub key: u32,
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
    /// Runtime-error block: never reformat these lines.
    pub runtime: bool,
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
    /// For admin ticket-action lines: the character name is replaced by a
    /// `Ticket #N` badge.
    pub admin_ticket: Option<u32>,
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
    /// Character-name ↔ player bindings collected from the log.
    pub characters: Vec<Character>,
    /// Rows shown when Dedup is enabled (consecutive tgui duplicates removed).
    pub dedup_indices: Vec<u32>,
    /// Character widths for the virtual columns in Format mode.
    pub format_columns: FormatColumns,
}

/// Widths (in characters) of the virtual columns in Format mode.
/// Uses the 98th percentile to keep the indent compact.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormatColumns {
    pub key_chars: usize,
    pub name_chars: usize,
    /// Widths of the admin PM header columns (`Ticket #N`, sender, recipient).
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
                // Not a line we understand; keep it visible instead of
                // dropping data.
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

/// Gathers every (character name, player) pair seen in the log, sorted by ckey.
fn collect_characters(rows: &[LogRow], keys: &[PlayerKey]) -> Vec<Character> {
    let mut seen: HashSet<(String, u32)> = HashSet::new();
    let mut characters = Vec::new();

    for row in rows {
        collect_characters_from(
            &row.message,
            &row.badges,
            &row.marks,
            keys,
            &mut seen,
            &mut characters,
        );
        if let Some(formatted) = &row.formatted {
            collect_characters_from(
                &formatted.text,
                &formatted.badges,
                &formatted.marks,
                keys,
                &mut seen,
                &mut characters,
            );
        }
    }

    characters.sort_by(|a, b| {
        keys[a.key as usize]
            .ckey
            .cmp(&keys[b.key as usize].ckey)
            .then_with(|| a.name.cmp(&b.name))
    });
    characters
}

fn collect_characters_from(
    text: &str,
    badges: &[TextBadge],
    marks: &[Mark],
    keys: &[PlayerKey],
    seen: &mut HashSet<(String, u32)>,
    characters: &mut Vec<Character>,
) {
    let mut push = |name: &str, key: u32| {
        if keys.get(key as usize).is_some_and(|player| !player.system)
            && seen.insert((name.to_owned(), key))
        {
            characters.push(Character {
                name: name.to_owned(),
                key,
            });
        }
    };

    for badge in badges {
        if let Some(range) = &badge.name {
            push(&text[range.clone()], badge.key);
        }
    }
    for mark in marks {
        if let MarkKind::AsCharacter { key, name, .. } = &mark.kind {
            push(&text[name.clone()], *key);
        }
    }
}

/// Marks known character names in salary lines such as
/// `275¢ were given to Name's account from income.`
fn add_currency_marks(rows: &mut [LogRow], name_map: &HashMap<String, u32>) {
    for row in rows.iter_mut() {
        let LogRow { message, marks, .. } = row;
        add_currency_marks_to(message, marks, name_map);

        if let Some(formatted) = &mut row.formatted {
            let FormattedMessage { text, marks, .. } = formatted;
            add_currency_marks_to(text, marks, name_map);
        }
    }
}

fn add_currency_marks_to(text: &str, marks: &mut Vec<Mark>, name_map: &HashMap<String, u32>) {
    const PREFIX: &str = "were given to ";
    const SUFFIX: &str = "'s account";

    let Some(prefix_at) = text.find(PREFIX) else {
        return;
    };
    let after_prefix = prefix_at + PREFIX.len();
    let Some(suffix_offset) = text[after_prefix..].rfind(SUFFIX) else {
        return;
    };

    let raw = &text[after_prefix..after_prefix + suffix_offset];
    let name_start = after_prefix + (raw.len() - raw.trim_start().len());
    let name_end = after_prefix + raw.trim_end().len();
    if name_start >= name_end {
        return;
    }

    if let Some(&key) = name_map.get(&text[name_start..name_end]) {
        marks.push(Mark {
            start: name_start,
            end: name_end,
            kind: MarkKind::Character {
                key,
                name: name_start..name_end,
            },
        });
        marks.sort_by_key(|mark| mark.start);
    }
}

/// Hides consecutive identical `... in tgui-window...` lines from the same
/// player when Dedup is enabled.
fn compute_tgui_dedup(rows: &[LogRow]) -> Vec<u32> {
    let mut visible = Vec::with_capacity(rows.len());
    let mut last: Option<(u32, usize)> = None;

    for (index, row) in rows.iter().enumerate() {
        if !row.message.contains(" in tgui-window") {
            last = None;
            visible.push(index as u32);
            continue;
        }

        let key = row
            .badges
            .iter()
            .find(|badge| badge.key_start == 0)
            .map(|badge| badge.key);

        if let (Some(key), Some((last_key, last_index))) = (key, last)
            && key == last_key
            && rows[last_index].message == row.message
        {
            continue;
        }

        last = key.map(|key| (key, index));
        visible.push(index as u32);
    }

    visible
}

fn compute_format_columns(rows: &[LogRow], keys: &[PlayerKey]) -> FormatColumns {
    let mut key_lengths = Vec::new();
    let mut name_lengths = Vec::new();

    for row in rows {
        if row.continuation {
            continue;
        }
        collect_format_lengths(
            &row.message,
            &row.badges,
            &row.marks,
            keys,
            &mut key_lengths,
            &mut name_lengths,
        );
        if let Some(formatted) = &row.formatted {
            collect_format_lengths(
                &formatted.text,
                &formatted.badges,
                &formatted.marks,
                keys,
                &mut key_lengths,
                &mut name_lengths,
            );
        }
    }

    key_lengths.sort_unstable();
    name_lengths.sort_unstable();

    let mut admin_ticket_chars = 0;
    let mut admin_sender_chars = 0;
    let mut admin_recipient_chars = 0;

    for row in rows {
        let Some(formatted) = &row.formatted else {
            continue;
        };
        for mark in &formatted.marks {
            if let MarkKind::AdminPm {
                ticket,
                sender,
                recipient,
            } = &mark.kind
            {
                if let Some(number) = ticket {
                    admin_ticket_chars =
                        admin_ticket_chars.max("Ticket #".len() + number.to_string().len());
                }
                if let Some(player) = keys.get(*sender as usize) {
                    admin_sender_chars = admin_sender_chars.max(player.ckey.chars().count());
                }
                if let Some(player) = keys.get(*recipient as usize) {
                    admin_recipient_chars = admin_recipient_chars.max(player.ckey.chars().count());
                }
            }
        }
    }

    FormatColumns {
        key_chars: percentile(&key_lengths, 98),
        name_chars: percentile(&name_lengths, 98),
        admin_ticket_chars,
        admin_sender_chars,
        admin_recipient_chars,
    }
}

/// 98th percentile keeps the virtual columns compact; a few very long names
/// simply overflow their slot instead of widening the whole table.
fn percentile(sorted: &[usize], percent: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() - 1) as f32 * percent as f32 / 100.0).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

fn collect_format_lengths(
    message: &str,
    badges: &[TextBadge],
    marks: &[Mark],
    keys: &[PlayerKey],
    key_lengths: &mut Vec<usize>,
    name_lengths: &mut Vec<usize>,
) {
    let Some(badge) = badges.first() else {
        return;
    };
    if badge.key_start != 0 {
        return;
    }
    let Some(player) = keys.get(badge.key as usize) else {
        return;
    };

    key_lengths.push(player.ckey.chars().count());

    let Some(range) = &badge.name else {
        return;
    };
    let name_chars = message[range.clone()].chars().count();
    let mut total = name_chars;

    // Include an immediately following mob ref in the same virtual column.
    let after_name = range.end + 1;
    if after_name <= message.len()
        && let Some(mark) = marks
            .iter()
            .find(|mark| matches!(mark.kind, MarkKind::Mob) && mark.start >= after_name)
        && message[after_name..mark.start]
            .chars()
            .all(|character| character.is_whitespace() || "()[]{}".contains(character))
    {
        total = name_chars + 1 + (mark.end - mark.start);
    }

    name_lengths.push(total);
}

#[allow(clippy::too_many_arguments)]
fn make_row(
    line_no: u32,
    time: Option<LogTime>,
    category: Category,
    message: &str,
    continuation: bool,
    runtime: bool,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> LogRow {
    let mut badges = parse_badges(message, keys, key_ids);
    badges.extend(parse_loose_keys(message, keys, key_ids));
    badges.sort_by_key(|badge| badge.key_start);

    let mut marks = parse_marks(message);
    marks.extend(parse_sensitive(message));
    marks.extend(parse_as_marks(message, keys, key_ids));
    marks.extend(parse_usr_marks(message, keys, key_ids));
    marks.sort_by_key(|mark| mark.start);

    let cleaned = clean_message(message);
    let looks_admin = cleaned != message
        || message.starts_with("PM: ")
        || message.starts_with("Ticket #")
        || message.starts_with("ADMINPRIVATE: ")
        || message.contains("икет #")
        || message.contains("icket #");

    let formatted = looks_admin.then(|| {
        let mut badges = parse_badges(&cleaned, keys, key_ids);
        badges.extend(parse_loose_keys(&cleaned, keys, key_ids));
        badges.sort_by_key(|badge| badge.key_start);

        let mut marks = parse_marks(&cleaned);
        marks.extend(parse_sensitive(&cleaned));
        marks.extend(parse_as_marks(&cleaned, keys, key_ids));
        marks.extend(parse_usr_marks(&cleaned, keys, key_ids));

        let admin_pm = parse_admin_pm(&cleaned, keys, key_ids);
        let has_admin_pm = admin_pm.is_some();
        marks.extend(admin_pm);

        if !has_admin_pm
            && badges.is_empty()
            && let Some((start, end, number)) = find_ticket(&cleaned)
        {
            marks.push(Mark {
                start,
                end,
                kind: MarkKind::TicketRef { number },
            });
        }

        marks.sort_by_key(|mark| mark.start);

        // Admin ticket actions: replace the character name with a ticket badge.
        let admin_ticket = if matches!(
            category,
            Category::Admin | Category::AdminPrivate | Category::Game
        ) {
            badges
                .first()
                .filter(|badge| badge.key_start == 0 && badge.name.is_some())
                .and_then(|_| find_ticket(&cleaned))
                .map(|(_, _, number)| number)
        } else {
            None
        };

        FormattedMessage {
            badges,
            marks,
            text: cleaned,
            admin_ticket,
        }
    });

    LogRow {
        line_no,
        time,
        category,
        message: message.to_owned(),
        continuation,
        runtime,
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

            // Emoji spans carry the emoji name in the CSS class
            // (`emoji-real` → `:real:`), Discord style.
            if let Some(class) = tag.find("emoji-") {
                let name = &tag[class + "emoji-".len()..];
                let length = name
                    .chars()
                    .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                    .count();
                if length > 0 {
                    out.push(':');
                    out.push_str(&name[..length]);
                    out.push(':');
                }
            }

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

    // In Format mode these technical prefixes are noise.
    for prefix in ["Mob Login: ", "TAG: "] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.trim_start().to_owned();
        }
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

/// Badges a plain leading ckey in lines like
/// `determineddoreen in tgui-window-1/close (suspending)`.
fn parse_loose_keys(
    message: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> Vec<TextBadge> {
    let Some(end) = message.find(" in tgui-window") else {
        return Vec::new();
    };
    let token = &message[..end];
    if token.is_empty() || !token.bytes().all(is_key_byte) {
        return Vec::new();
    }

    let key = intern_key(token, keys, key_ids);
    vec![TextBadge {
        key,
        key_start: 0,
        key_end: end,
        name: None,
    }]
}

/// Finds `ckey (as Character Name at x,y,z)` blocks.
fn parse_as_marks(
    message: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> Vec<Mark> {
    const AS: &str = " (as ";
    let mut marks = Vec::new();
    let mut search = 0;

    while let Some(offset) = message[search..].find(AS) {
        let space = search + offset;
        let after_as = space + AS.len();
        let bytes = message.as_bytes();

        let mut key_start = space;
        while key_start > 0 && is_key_byte(bytes[key_start - 1]) {
            key_start -= 1;
        }
        if message[..space].ends_with("*no key*") {
            key_start = space - "*no key*".len();
        }
        if key_start == space {
            search = after_as;
            continue;
        }

        let Some(at_offset) = message[after_as..].find(" at ") else {
            search = after_as;
            continue;
        };
        let at = after_as + at_offset;

        let raw_name = &message[after_as..at];
        let name_start = after_as + (raw_name.len() - raw_name.trim_start().len());
        let name_end = after_as + raw_name.trim_end().len();
        if name_start >= name_end {
            search = after_as;
            continue;
        }

        let Some((coords_end, coords)) = parse_coords_values(message, at + 4) else {
            search = after_as;
            continue;
        };
        let after_coords = &message[coords_end..];
        let trimmed = after_coords.trim_start();
        if !trimmed.starts_with(')') {
            search = after_as;
            continue;
        }
        let end = coords_end + (after_coords.len() - trimmed.len()) + 1;

        let key = intern_key(&message[key_start..space], keys, key_ids);
        marks.push(Mark {
            start: key_start,
            end,
            kind: MarkKind::AsCharacter {
                key,
                key_end: space,
                name: name_start..name_end,
                coords,
            },
        });
        search = end;
    }

    marks
}

/// Finds the client-tracking blocks `ckey (usr:Name[x,y,z]) : payload`.
fn parse_usr_marks(
    message: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> Vec<Mark> {
    const USR: &str = " (usr:";
    let mut marks = Vec::new();
    let mut search = 0;

    while let Some(offset) = message[search..].find(USR) {
        let space = search + offset;
        let after_usr = space + USR.len();
        let bytes = message.as_bytes();

        let mut key_start = space;
        while key_start > 0 && is_key_byte(bytes[key_start - 1]) {
            key_start -= 1;
        }
        if message[..space].ends_with("*no key*") {
            key_start = space - "*no key*".len();
        }
        if key_start == space {
            search = after_usr;
            continue;
        }

        let Some(bracket_offset) = message[after_usr..].find('[') else {
            search = after_usr;
            continue;
        };
        let bracket = after_usr + bracket_offset;

        let raw_name = &message[after_usr..bracket];
        let name_start = after_usr + (raw_name.len() - raw_name.trim_start().len());
        let name_end = after_usr + raw_name.trim_end().len();
        if name_start >= name_end {
            search = after_usr;
            continue;
        }

        let mut coords_start = bracket + 1;
        if message[coords_start..].starts_with('(') {
            coords_start += 1;
        }

        let Some((coords_end, coords)) = parse_coords_values(message, coords_start) else {
            search = after_usr;
            continue;
        };
        let mut idx = coords_end;
        if message[idx..].starts_with(')') {
            idx += 1;
        }
        let Some(after_bracket) = message[idx..].strip_prefix(']') else {
            search = after_usr;
            continue;
        };
        let idx = idx + 1;
        let trimmed = after_bracket.trim_start();
        if !trimmed.starts_with(')') {
            search = after_usr;
            continue;
        }
        let end = idx + (after_bracket.len() - trimmed.len()) + 1;

        let key = intern_key(&message[key_start..space], keys, key_ids);
        marks.push(Mark {
            start: key_start,
            end,
            kind: MarkKind::AsCharacter {
                key,
                key_end: space,
                name: name_start..name_end,
                coords,
            },
        });
        search = end;
    }

    marks
}

/// Parses `PM: sender → recipient: text` and
/// `Ticket #N: sender → recipient: text` headers.
fn parse_admin_pm(
    message: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> Option<Mark> {
    let (offset, rest) = match message.strip_prefix("ADMINPRIVATE: ") {
        Some(rest) => ("ADMINPRIVATE: ".len(), rest),
        None => (0, message),
    };

    let (ticket, header_len) = if let Some(after) = rest.strip_prefix("Ticket #") {
        let colon = after.find(": ")?;
        let number = after[..colon].parse().ok()?;
        (Some(number), "Ticket #".len() + colon + 2)
    } else if rest.starts_with("PM: ") {
        (None, "PM: ".len())
    } else {
        return None;
    };

    let header_start = offset + header_len;
    let header = message.get(header_start..)?;

    let arrow = header.find(" → ")?;
    let sender_text = &header[..arrow];
    let after_arrow = &header[arrow + " → ".len()..];
    let colon = after_arrow.find(':')?;
    let recipient_text = &after_arrow[..colon];

    if !looks_like_key(sender_text) || !looks_like_key(recipient_text) {
        return None;
    }

    let sender = intern_key(sender_text, keys, key_ids);
    let recipient = intern_key(recipient_text, keys, key_ids);
    let mut end = header_start + arrow + " → ".len() + colon + 1;
    if message[end..].starts_with(' ') {
        end += 1;
    }

    Some(Mark {
        start: 0,
        end,
        kind: MarkKind::AdminPm {
            ticket,
            sender,
            recipient,
        },
    })
}

fn looks_like_key(text: &str) -> bool {
    !text.is_empty()
        && !text.chars().any(char::is_whitespace)
        && text.chars().any(|character| character.is_alphanumeric())
}

/// Marks IPv4 addresses (and a following `-CID` token) as sensitive.
fn parse_sensitive(message: &str) -> Vec<Mark> {
    let bytes = message.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() || (index > 0 && bytes[index - 1].is_ascii_digit()) {
            index += 1;
            continue;
        }

        let Some(ip_end) = parse_ipv4(bytes, index) else {
            index += 1;
            continue;
        };

        let mut end = ip_end;
        if end < bytes.len() && bytes[end] == b'-' {
            let digits = end + 1;
            let mut after = digits;
            while after < bytes.len() && bytes[after].is_ascii_digit() {
                after += 1;
            }
            if after - digits >= 4 {
                end = after;
            }
        }

        if end == bytes.len() || !bytes[end].is_ascii_alphanumeric() {
            marks.push(Mark {
                start: index,
                end,
                kind: MarkKind::Sensitive { value: index..end },
            });
            index = end;
            continue;
        }

        index += 1;
    }

    marks
}

fn parse_ipv4(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;

    for octet in 0..4 {
        let digits_start = index;
        let mut value = 0_u32;
        while index < bytes.len() && bytes[index].is_ascii_digit() && index - digits_start < 3 {
            value = value * 10 + u32::from(bytes[index] - b'0');
            index += 1;
        }
        if index == digits_start || value > 255 {
            return None;
        }
        if octet < 3 {
            if index >= bytes.len() || bytes[index] != b'.' {
                return None;
            }
            index += 1;
        }
    }

    Some(index)
}

/// Finds `тикет #2`, `Тикет #2`, `ticket #2` or `Ticket #2`.
fn find_ticket(text: &str) -> Option<(usize, usize, u32)> {
    for prefix in ["тикет #", "Тикет #", "ticket #", "Ticket #"] {
        if let Some(start) = text.find(prefix) {
            let digits_start = start + prefix.len();
            let digits_len = text[digits_start..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .count();
            if digits_len > 0 {
                let number = text[digits_start..digits_start + digits_len].parse().ok()?;
                return Some((start, digits_start + digits_len, number));
            }
        }
    }
    None
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

    // Keys may carry a status tag: `Dorman[DC]` or `@Mrsnake[DC]`.
    let mut token_end = slash;
    if message[..slash].ends_with(']')
        && let Some(tag_start) = message[..slash - 1].rfind('[')
    {
        let tag = &message[tag_start + 1..slash - 1];
        if !tag.is_empty()
            && tag.len() <= 6
            && tag
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            token_end = tag_start;
        }
    }

    let mut start = token_end;
    while start > 0 && is_key_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == token_end {
        return (slash, slash);
    }

    // Optional `@` marker in front of the key.
    if start > 0 && bytes[start - 1] == b'@' {
        start -= 1;
    }

    // Don't badge text that belongs to an HTML tag such as `</a>`.
    if start > 0 && bytes[start - 1] == b'>' {
        return (slash, slash);
    }

    (start, slash)
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// Finds mob references, `(Location (x,y,z))` and radio frequencies.
fn parse_marks(message: &str) -> Vec<Mark> {
    let mut marks = find_mobs(message);
    marks.extend(find_locations(message));
    marks.extend(find_frequencies(message));
    marks.sort_by_key(|mark| mark.start);
    marks
}

/// Matches `[135.9]` and the ` (spans: command_headset )` tail that follows it.
fn find_frequencies(message: &str) -> Vec<Mark> {
    const SPANS_PREFIX: &str = " (spans:";

    let bytes = message.as_bytes();
    let mut marks = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'[' {
            index += 1;
            continue;
        }

        let value_start = index + 1;
        let Some(value_end) = parse_frequency(message, value_start) else {
            index += 1;
            continue;
        };
        if !message[value_end..].starts_with(']') {
            index += 1;
            continue;
        }

        let mut end = value_end + 1;
        let mut spans = None;

        // The channel classes ride along in the following `(spans: …)` block.
        if let Some(rest) = message[end..].strip_prefix(SPANS_PREFIX)
            && let Some(close) = rest.find(')')
        {
            let spans_end = end + SPANS_PREFIX.len() + close + 1;
            spans = Some(end..spans_end);
            end = spans_end;
        }

        marks.push(Mark {
            start: index,
            end,
            kind: MarkKind::Frequency {
                value: value_start..value_end,
                spans,
            },
        });
        index = end;
    }

    marks
}

/// Parses `123.4` starting at `start` and returns the offset after the value.
fn parse_frequency(message: &str, start: usize) -> Option<usize> {
    let bytes = message.as_bytes();
    let mut index = start;
    let mut seen_dot = false;
    let mut seen_digit = false;

    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            seen_digit = true;
        } else if bytes[index] == b'.' {
            if seen_dot || !seen_digit {
                return None;
            }
            seen_dot = true;
        } else {
            break;
        }
        index += 1;
    }

    (seen_digit && seen_dot).then_some(index)
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
    let (end, range) = parse_coords_values(message, open + 1)?;
    if message[end..].starts_with(')') {
        Some((end + 1, range))
    } else {
        None
    }
}

/// Parses `x,y,z` starting at `start` and returns the end offset plus the range.
fn parse_coords_values(message: &str, start: usize) -> Option<(usize, Range<usize>)> {
    let bytes = message.as_bytes();
    let mut index = start;

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

    Some((index, start..index))
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
            existing.key = clean_key(text).to_owned();
        }
        return id;
    }

    let id = keys.len() as u32;
    key_ids.insert(ckey.clone(), id);
    keys.push(PlayerKey {
        key: clean_key(text).to_owned(),
        ckey,
        system,
    });
    id
}

/// Strips decorations used in the log (`@`, `[DC]`) to get the account key.
fn clean_key(key: &str) -> &str {
    let key = key.strip_prefix('@').unwrap_or(key);
    key.split_once('[').map_or(key, |(name, _)| name)
}

/// BYOND ckeys are lowercase and have no spaces: `Vladislav Vorobyov` becomes
/// `vladislavvorobyov`.
fn canonical_ckey(key: &str) -> String {
    clean_key(key)
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Prefer the variant that looks most like a human-readable key.
fn key_quality(key: &str) -> (usize, usize) {
    let key = clean_key(key);
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
                MarkKind::Mob
                | MarkKind::AsCharacter { .. }
                | MarkKind::Character { .. }
                | MarkKind::Frequency { .. }
                | MarkKind::AdminPm { .. }
                | MarkKind::TicketRef { .. }
                | MarkKind::Sensitive { .. } => None,
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
                MarkKind::Mob
                | MarkKind::AsCharacter { .. }
                | MarkKind::Character { .. }
                | MarkKind::Frequency { .. }
                | MarkKind::AdminPm { .. }
                | MarkKind::TicketRef { .. }
                | MarkKind::Sensitive { .. } => None,
            })
            .unwrap();
        assert_eq!(&formatted.text[name], "Laser Tag Arena");
        assert_eq!(&formatted.text[coords], "84,109,2");
    }

    #[test]
    fn computes_format_virtual_columns() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:24:02.500] SAY: abc/(Short Name) (mob_1) \"hello\"\n",
            "[2026-10-17 17:24:03.500] SAY: longkey_123/(Very Long Name) (mob_12345) \"hi\"\n",
            "[2026-10-17 17:24:04.500] MISC: no player here\n",
        ));

        assert_eq!(log.format_columns.key_chars, "longkey_123".len());
        assert_eq!(
            log.format_columns.name_chars,
            "Very Long Name".len() + 1 + "mob_12345".len()
        );
    }

    #[test]
    fn formats_usr_client_lines() {
        let log = LogFile::parse(
            "[2026-10-17 17:23:57.542] MISC: Golub_lesnoi (usr:Максим Думкопф[(149,133,3)]) : commandbar_typing=1\n",
        );
        let row = &log.rows[0];
        let mark = row
            .marks
            .iter()
            .find_map(|mark| match &mark.kind {
                MarkKind::AsCharacter {
                    key,
                    key_end,
                    name,
                    coords,
                } => Some((*key, *key_end, name.clone(), coords.clone())),
                _ => None,
            })
            .unwrap();

        assert_eq!(log.keys[mark.0 as usize].ckey, "golub_lesnoi");
        assert_eq!(&row.message[..mark.1], "Golub_lesnoi");
        assert_eq!(&row.message[mark.2], "Максим Думкопф");
        assert_eq!(&row.message[mark.3], "149,133,3");
    }

    #[test]
    fn finds_radio_frequencies() {
        let log = LogFile::parse(
            "[2026-10-17 17:33:34.268] SAY: dogusu/(Анастасия Гудкова) (mob_3381) [145.9] (spans:  command_headset ) \"yo\" (brig (10,20,2))\n",
        );
        let row = &log.rows[0];

        let frequency = row
            .marks
            .iter()
            .find_map(|mark| match &mark.kind {
                MarkKind::Frequency { value, spans } => Some((value.clone(), spans.clone())),
                _ => None,
            })
            .expect("frequency mark");

        assert_eq!(&row.message[frequency.0], "145.9");
        assert_eq!(
            frequency
                .1
                .as_ref()
                .map(|range| &row.message[range.clone()]),
            Some(" (spans:  command_headset )")
        );
    }

    #[test]
    fn parses_dead_chat_key_suffix() {
        let log = LogFile::parse(concat!(
            "[2026-10-17 17:38:15.643] GAME: Dorman[DC]/(Dorman) (mob_3421) is no longer owning mob\n",
            "[2026-10-17 18:10:33.662] ATTACK: @Mrsnake[DC]/(Магуро Хамати) grabbed someone\n",
        ));

        assert_eq!(log.keys[0].ckey, "dorman");
        assert_eq!(log.keys[0].key, "Dorman");
        assert_eq!(
            &log.rows[0].message[log.rows[0].badges[0].key_start..log.rows[0].badges[0].key_end],
            "Dorman[DC]"
        );

        assert_eq!(log.keys[1].ckey, "mrsnake");
        assert_eq!(log.keys[1].key, "Mrsnake");
        assert_eq!(
            &log.rows[1].message[log.rows[1].badges[0].key_start..log.rows[1].badges[0].key_end],
            "@Mrsnake[DC]"
        );
        let name = log.rows[1].badges[0].name.clone().unwrap();
        assert_eq!(&log.rows[1].message[name], "Магуро Хамати");
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
