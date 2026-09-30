//! Keyboard targets for the visible mailbox projection, independent of widgets.

use crate::{MailListEntry, MailNavigationTarget};
use slint::{Model, ModelRc, SharedString};

pub(super) fn target(
    entries: ModelRc<MailListEntry>,
    current_id: i32,
    movement: SharedString,
    step: i32,
) -> MailNavigationTarget {
    let mut messages = Vec::new();
    let mut headers = 0;
    let mut selected = None;
    for index in 0..entries.row_count() {
        let Some(entry) = entries.row_data(index) else {
            continue;
        };
        if entry.is_header {
            headers += 1;
        } else if entry.show_row {
            if entry.email.selected {
                selected = Some(messages.len());
            }
            messages.push(MailNavigationTarget {
                id: entry.email.id,
                entry_index: index as i32,
                preceding_headers: headers,
                preceding_messages: messages.len() as i32,
                checked: entry.email.checked,
            });
        }
    }
    if messages.is_empty() {
        return MailNavigationTarget {
            id: -1,
            entry_index: -1,
            ..Default::default()
        };
    }
    let current = messages
        .iter()
        .position(|row| row.id == current_id)
        .or(selected);
    let last = messages.len() - 1;
    let step = step.max(1) as usize;
    let index = match movement.as_str() {
        "next" => current.map_or(0, |index| index.saturating_add(step).min(last)),
        "previous" => current.map_or(last, |index| index.saturating_sub(step)),
        "first" => 0,
        "last" => last,
        _ => current.unwrap_or(0),
    };
    messages.swap_remove(index)
}
