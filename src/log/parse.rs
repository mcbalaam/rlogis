use super::admin::*;
use super::badges::*;
use super::marks::*;
use super::sanitize::*;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_row(
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

/// Strips HTML tags, decodes entities and collapses whitespace
pub(crate) fn split_header(line: &str) -> Option<(LogTime, &str)> {
    if !line.starts_with('[') {
        return None;
    }
    let header_end = line.find("] ")?;
    let time = LogTime::parse(line.get(1..header_end)?)?;
    Some((time, line.get(header_end + 2..)?))
}

/// Splits `MISC: text` into a category and the message
pub(crate) fn split_category(rest: &str) -> (Category, &str) {
    if let Some((tag, message)) = rest.split_once(": ")
        && !tag.is_empty()
        && tag.len() <= 16
        && tag.bytes().all(|byte| byte.is_ascii_uppercase())
    {
        return (Category::from_tag(tag), message);
    }
    (Category::Other, rest)
}
