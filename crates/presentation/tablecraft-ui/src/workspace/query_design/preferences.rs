//! Reopening retains only source-free design choices, never metadata or execution rights.
use super::{Mode, QueryDesign, Session};
use tablecraft_builders::query::QueryDesignDraft;

pub(in crate::workspace) struct Preferences {
    draft: QueryDesignDraft,
    mode: Mode,
    revision: u64,
    saved_revision: u64,
    field_inputs: super::field_inputs::FieldInputs,
}

impl QueryDesign {
    pub(in crate::workspace) fn preferences(&self) -> Option<Preferences> {
        let session = self.session.as_ref()?;
        Some(Preferences {
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            draft: session.draft.clone(),
            mode: session.mode,
            revision: session.revision,
            saved_revision: session.saved_revision,
            field_inputs: session.field_inputs.snapshot(),
        })
    }

    pub(in crate::workspace) fn from_preferences(preferences: Preferences) -> Self {
        // Old results cannot be represented by a fresh-source receipt.
        let mode = match preferences.mode {
            Mode::Results => Mode::Design,
            mode => mode,
        };
        Self {
            session: Some(Box::new(Session {
                draft: preferences.draft,
                mode,
                revision: preferences.revision,
                saved_revision: preferences.saved_revision,
                field_inputs: preferences.field_inputs,
                ..Session::default()
            })),
        }
    }
}
