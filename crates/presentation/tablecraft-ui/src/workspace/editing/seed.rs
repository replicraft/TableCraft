//! Source-bound proposals seed a checked edit; they never grant save authority.
use super::{EditDraft, EditInput, EditTarget, InputHint};
use crate::{CellKind, DisplayControl, workspace::State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BooleanToggle {
    expected: bool,
    proposed: bool,
}

impl BooleanToggle {
    pub(crate) fn opposite(expected: bool) -> Self {
        Self {
            expected,
            proposed: !expected,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EditSeed {
    Text(String),
    Boolean(BooleanToggle),
}

impl EditSeed {
    pub(in crate::workspace) fn input(self, draft: &EditDraft) -> Option<EditInput> {
        match self {
            Self::Text(text) => Some(EditInput {
                text,
                is_null: false,
            }),
            Self::Boolean(toggle) => {
                if draft.input_hint != InputHint::Boolean
                    || draft.nullable
                    || draft.initial.is_null
                    || !matches!(draft.original.kind, CellKind::Scalar {
                        is_null: false,
                        stored_calculation: false,
                        boolean: Some(value),
                        display_control: DisplayControl::Uninspected | DisplayControl::Absent | DisplayControl::Native(106),
                    } if value == toggle.expected)
                {
                    return None;
                }
                Some(EditInput {
                    text: if toggle.proposed { "true" } else { "false" }.into(),
                    is_null: false,
                })
            }
        }
    }
}

impl State {
    pub(in crate::workspace) fn boolean_toggle_context(&self) -> bool {
        if self.busy || self.edit_dialog_open() {
            return false;
        }
        let Some(data) = &self.data else {
            return false;
        };
        let Some(revision) = data.source_revision else {
            return false;
        };
        let Some(ordinal) = data.parent_ordinal(0) else {
            return false;
        };
        data.columns
            .iter()
            .any(|column| self.can_edit_cell_at(revision, &data.table, &column.name, ordinal))
    }

    pub fn can_begin_seeded_edit(&self, target: &EditTarget, seed: &EditSeed) -> bool {
        match seed {
            EditSeed::Text(text) => self.can_paste_text(target, text),
            EditSeed::Boolean(toggle) => {
                if self.busy || self.edit_dialog_open() || !self.can_edit_cell(target) {
                    return false;
                }
                self.data.as_ref().and_then(|data| {
                    let row = data.row_index(target.parent_ordinal)?;
                    let column = data.column_index(&target.column)?;
                    let cell = data.rows.get(row)?.get(column)?;
                    cell.toggle_value()
                }) == Some(toggle.expected)
            }
        }
    }

    pub fn begin_seeded_edit(
        &mut self,
        target: EditTarget,
        seed: EditSeed,
        request_id: u64,
    ) -> bool {
        if !self.can_begin_seeded_edit(&target, &seed) {
            return false;
        }
        self.begin_edit(target, request_id);
        if let Some(super::EditDialog::Checking { seed: pending, .. }) = &mut self.edit {
            *pending = Some(seed);
            true
        } else {
            false
        }
    }
}
