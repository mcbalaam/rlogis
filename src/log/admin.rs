use super::keys::*;
use super::*;

/// Gathers every (character name, player) pair seen in the log, sorted by ckey
pub(crate) fn collect_characters(rows: &[LogRow], keys: &[PlayerKey]) -> Vec<Character> {
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

pub(crate) fn collect_characters_from(
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

/// Marks known character names in salary lines
pub(crate) fn add_currency_marks(rows: &mut [LogRow], name_map: &HashMap<String, u32>) {
    for row in rows.iter_mut() {
        let LogRow { message, marks, .. } = row;
        add_currency_marks_to(message, marks, name_map);

        if let Some(formatted) = &mut row.formatted {
            let FormattedMessage { text, marks, .. } = formatted;
            add_currency_marks_to(text, marks, name_map);
        }
    }
}

pub(crate) fn add_currency_marks_to(
    text: &str,
    marks: &mut Vec<Mark>,
    name_map: &HashMap<String, u32>,
) {
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

pub(crate) fn parse_admin_pm(
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

pub(crate) fn looks_like_key(text: &str) -> bool {
    !text.is_empty()
        && !text.chars().any(char::is_whitespace)
        && text.chars().any(|character| character.is_alphanumeric())
}

pub(crate) fn find_ticket(text: &str) -> Option<(usize, usize, u32)> {
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
