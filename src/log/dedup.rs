use super::*;

/// Hides consecutive identical `... in tgui-window...` lines from the same
/// player when Dedup is enabled.
pub(crate) fn compute_tgui_dedup(rows: &[LogRow]) -> Vec<u32> {
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
