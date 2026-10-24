use super::*;

/// Strips decorations used in the log (`@`, `[DC]`) to get the account key.
pub(crate) fn clean_key(key: &str) -> &str {
    let key = key.strip_prefix('@').unwrap_or(key);
    key.split_once('[').map_or(key, |(name, _)| name)
}

/// BYOND ckeys are lowercase and contain only alphanumerics
pub(crate) fn canonical_ckey(key: &str) -> String {
    clean_key(key)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Prefer the variant that looks most like a human-readable key (canonical)
pub(crate) fn key_quality(key: &str) -> (usize, usize) {
    let key = clean_key(key);
    let spaces = key.bytes().filter(|byte| *byte == b' ').count();
    let uppercase = key
        .chars()
        .filter(|character| character.is_uppercase())
        .count();
    (spaces, uppercase)
}

pub(crate) fn intern_key(
    text: &str,
    keys: &mut Vec<PlayerKey>,
    key_ids: &mut HashMap<String, u32>,
) -> u32 {
    let system = text.eq_ignore_ascii_case("*no key*");
    let ckey = if system {
        "*no key*".to_owned()
    } else {
        let canonical = canonical_ckey(text);
        if canonical.is_empty() {
            clean_key(text).to_lowercase()
        } else {
            canonical
        }
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

/// Strips decorations used in the log to get the account key
pub(crate) fn key_range(message: &str, slash: usize, first_match: bool) -> (usize, usize) {
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

    // Optional `@` marker in front of the key
    if start > 0 && bytes[start - 1] == b'@' {
        start -= 1;
    }

    // Don't badge text that belongs to an HTML tag such as `</a>`
    if start > 0 && bytes[start - 1] == b'>' {
        return (slash, slash);
    }

    (start, slash)
}

pub(crate) fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}
