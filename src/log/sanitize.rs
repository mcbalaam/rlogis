use super::*;

/// Strips HTML tags, decodes entities and collapses whitespace
pub(crate) fn clean_message(message: &str) -> String {
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

    // In Format mode these technical prefixes are noise
    for prefix in ["Mob Login: ", "TAG: "] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.trim_start().to_owned();
        }
    }

    out
}

pub(crate) fn push_normalized(out: &mut String, character: char) {
    if character.is_whitespace() {
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    } else {
        out.push(character);
    }
}

pub(crate) fn extract_priv_msg(tag: &str) -> Option<&str> {
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

pub(crate) fn find_closing_anchor(text: &str) -> Option<usize> {
    text.as_bytes()
        .windows(4)
        .position(|window| window.eq_ignore_ascii_case(b"</a>"))
}

pub(crate) fn decode_entity(text: &str) -> Option<(char, usize)> {
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
/// Marks IPv4 addresses (and a following `-CID` token) as sensitive.
pub(crate) fn parse_sensitive(message: &str) -> Vec<Mark> {
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

pub(crate) fn parse_ipv4(bytes: &[u8], start: usize) -> Option<usize> {
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
