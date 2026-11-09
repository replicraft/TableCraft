//! Keyboard object intents retain the exact active instance through the frame.
use super::{ObjectTab, State};
use crate::keyboard::{self, ObjectShortcut};
use crate::workspace::Action;
use egui::{Popup, Ui};

pub(in crate::workspace) struct PendingShortcut {
    active: ObjectTab,
    target: ObjectTab,
    publication: u64,
    close: bool,
}

impl State {
    pub(in crate::workspace) fn begin_object_shortcut(&self, ui: &Ui) -> Option<PendingShortcut> {
        // Consume known chords even when blocked so toolkit/native defaults
        // cannot delete text or close the application behind a modal/save.
        let shortcut = keyboard::consume_object_shortcut(ui.ctx())?;
        let naming_transition = self
            .active_object()
            .is_some_and(|tab| self.can_request_field_addition_transition(&tab));
        if !ui.is_enabled()
            || (!self.object_switch_ready() && !naming_transition)
            || Popup::is_any_open(ui.ctx())
        {
            return None;
        }
        let active = self.active_object()?;
        let target = match shortcut {
            ObjectShortcut::Close => active.clone(),
            ObjectShortcut::Next | ObjectShortcut::Previous => {
                if self.objects.entries.len() < 2 {
                    return None;
                }
                let current = self
                    .objects
                    .entries
                    .iter()
                    .position(|entry| entry.tab == active)?;
                let count = self.objects.entries.len();
                let next = if shortcut == ObjectShortcut::Next {
                    (current + 1) % count
                } else {
                    (current + count - 1) % count
                };
                self.objects.entries[next].tab.clone()
            }
        };
        Some(PendingShortcut {
            active,
            target,
            publication: self.focus_publication,
            close: shortcut == ObjectShortcut::Close,
        })
    }

    pub(in crate::workspace) fn finish_object_shortcut(
        &self,
        ui: &Ui,
        pending: Option<PendingShortcut>,
        actions: &mut Vec<Action>,
    ) {
        let Some(pending) = pending else {
            return;
        };
        if !actions.is_empty()
            || !ui.is_enabled()
            || Popup::is_any_open(ui.ctx())
            || self.focus_publication != pending.publication
            || self.active_object().as_ref() != Some(&pending.active)
        {
            return;
        }
        if pending.close {
            if self.can_close_object(&pending.target)
                || self.can_request_field_addition_transition(&pending.target)
            {
                actions.push(Action::CloseObject {
                    tab: pending.target,
                });
            }
        } else if self.can_activate_object(&pending.target)
            || self.can_request_field_addition_transition(&pending.target)
        {
            actions.push(Action::ActivateObject {
                tab: pending.target,
            });
        }
    }
}

#[cfg(test)]
mod tests;
