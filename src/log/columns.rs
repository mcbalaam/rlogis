use super::*;

pub(crate) fn compute_format_columns(rows: &[LogRow], keys: &[PlayerKey]) -> FormatColumns {
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

pub(crate) fn percentile(sorted: &[usize], percent: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() - 1) as f32 * percent as f32 / 100.0).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

pub(crate) fn collect_format_lengths(
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
