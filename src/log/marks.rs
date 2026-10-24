use super::keys::*;
use super::*;

/// Finds mob references, `(Location (x,y,z))` and radio frequencies
pub(crate) fn parse_marks(message: &str) -> Vec<Mark> {
    let mut marks = find_mobs(message);
    marks.extend(find_locations(message));
    marks.extend(find_frequencies(message));
    marks.sort_by_key(|mark| mark.start);
    marks
}

pub(crate) fn find_mobs(message: &str) -> Vec<Mark> {
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

/// Matches `(Location Name (x,y,z))`
pub(crate) fn find_locations(message: &str) -> Vec<Mark> {
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

/// Matches `[135.9]` and the ` (spans: command_headset )` tail that follows it
pub(crate) fn find_frequencies(message: &str) -> Vec<Mark> {
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

/// Parses `123.4`
pub(crate) fn parse_frequency(message: &str, start: usize) -> Option<usize> {
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

pub(crate) fn parse_coords(message: &str, open: usize) -> Option<(usize, Range<usize>)> {
    let (end, range) = parse_coords_values(message, open + 1)?;
    if message[end..].starts_with(')') {
        Some((end + 1, range))
    } else {
        None
    }
}

/// Parses `x,y,z`
pub(crate) fn parse_coords_values(message: &str, start: usize) -> Option<(usize, Range<usize>)> {
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

pub(crate) fn is_location_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name.chars().any(char::is_alphanumeric)
        && !name.chars().any(|character| "()[]{}<>".contains(character))
}
