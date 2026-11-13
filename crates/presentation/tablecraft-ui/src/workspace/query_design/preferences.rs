//! Reopening retains only source-free design choices, never metadata or execution rights.
use super::{
    Mode, QueryDesign, Session,
    baseline::{Baseline, Submission},
};
use tablecraft_builders::query::QueryDesignDraft;

pub(in crate::workspace) struct Preferences {
    draft: QueryDesignDraft,
    mode: Mode,
    revision: u64,
    baseline: Baseline,
    submitted: Option<Submission>,
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
            baseline: session.baseline.share(),
            // NO_CLONE: Fresh-source preferences retain the accepted immutable
            // submission while the live editor/pending worker keep other handles.
            submitted: session.submitted.as_ref().map(Submission::share),
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
                baseline: preferences.baseline,
                submitted: preferences.submitted,
                field_inputs: preferences.field_inputs,
                ..Session::default()
            })),
        }
    }
}
