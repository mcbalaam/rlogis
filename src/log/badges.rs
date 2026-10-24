use super::keys::*;
use super::marks::*;
use super::*;

/// Finds every `key/(Character Name)` match in a message and interns the key
pub(crate) fn parse_badges(
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

/// Badges a plain leading ckey in lines
pub(crate) fn parse_loose_keys(
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

/// Finds `ckey (as Character Name at x,y,z)` blocks
pub(crate) fn parse_as_marks(
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

/// Finds the client-tracking blocks `ckey (usr:Name[x,y,z]) : payload`
pub(crate) fn parse_usr_marks(
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
