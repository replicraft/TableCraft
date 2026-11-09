//! Presentation coordinates follow only their own acknowledged save/read chain.
use super::{Desktop, Editing};
use tablecraft_ui::EditTarget;

pub(super) enum Restore {
    Save { save_id: u64, target: EditTarget },
    Open { request_id: u64, target: EditTarget },
    Page { request_id: u64, target: EditTarget },
}

impl Editing {
    pub(super) fn take_saved_cell_selection(&mut self, id: u64) -> Option<EditTarget> {
        match self.cell_selection.take()? {
            Restore::Save { save_id, target } if save_id == id => Some(target),
            _ => None,
        }
    }

    pub(super) fn take_reopened_cell_selection(&mut self, id: u64) -> Option<EditTarget> {
        match self.cell_selection.take()? {
            Restore::Open { request_id, target } if request_id == id => Some(target),
            _ => None,
        }
    }
}

impl Desktop {
    pub(in crate::workspace) fn restore_saved_cell_selection(&mut self, generation: u64) {
        let Some(Restore::Page { request_id, target }) = self.editing.cell_selection.take() else {
            return;
        };
        if request_id == generation {
            self.state.select_reopened_edit_cell(&target);
        }
    }
}
