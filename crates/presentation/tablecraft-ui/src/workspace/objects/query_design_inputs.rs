//! Close/restart guards inspect real active and parked QBE authoring data.
use super::session::Session as ObjectSession;
use crate::workspace::{ObjectKind, ObjectTab, ObjectTarget, State};

impl State {
    /// Unresolved QBE text is user data in either the active or a parked SQL tab.
    pub fn has_unresolved_query_design_edits(&self) -> bool {
        self.query_design.has_unresolved_field_inputs()
            || self
                .objects
                .entries
                .iter()
                .any(|entry| match &entry.session {
                    Some(ObjectSession::Sql { design, .. }) => design.has_unresolved_field_inputs(),
                    Some(ObjectSession::Saved { state, .. }) => {
                        state.design.has_unresolved_field_inputs()
                    }
                    _ => false,
                })
    }

    pub(in crate::workspace) fn unresolved_query_design_edits_for(&self, tab: &ObjectTab) -> bool {
        if !matches!(
            tab.target.kind,
            ObjectKind::Sql | ObjectKind::SavedQuery { .. }
        ) {
            return false;
        }
        if self.active_object().as_ref() == Some(tab) {
            return self.query_design.has_unresolved_field_inputs();
        }
        self.objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)
            .is_some_and(|entry| match &entry.session {
                Some(ObjectSession::Sql { design, .. }) => design.has_unresolved_field_inputs(),
                Some(ObjectSession::Saved { state, .. }) => {
                    state.design.has_unresolved_field_inputs()
                }
                _ => false,
            })
    }

    /// Reveal retained input that must be finished or explicitly cancelled before close.
    /// Returns whether such input exists; no query or native operation is dispatched.
    pub fn reveal_unresolved_query_design_edits(&mut self) -> bool {
        let selected = self
            .objects
            .entries
            .iter()
            .find(|entry| self.unresolved_query_design_edits_for(&entry.tab));
        let target = selected.map(|entry| ObjectTab {
            id: entry.tab.id,
            target: ObjectTarget {
                document_revision: entry.tab.target.document_revision,
                // NO_CLONE: The reveal target must survive activation mutating the object registry.
                kind: entry.tab.target.kind.clone(),
            },
        });
        let Some(target) = target else {
            return false;
        };
        self.reveal_unresolved_query_design_edits_for(&target)
    }

    pub(in crate::workspace) fn reveal_unresolved_query_design_edits_for(
        &mut self,
        target: &ObjectTab,
    ) -> bool {
        if !self.unresolved_query_design_edits_for(target) {
            return false;
        }
        if self.active_object().as_ref() != Some(target) && !self.activate_object(target) {
            self.error = Some(self.localizer.text("qbe-finish-field-edit"));
            return true;
        }
        let reason = self.localizer.text("qbe-finish-field-edit");
        if self.query_design.reveal_unresolved_input(reason) {
            self.ribbon_tab = crate::workspace::ribbon::Tab::QueryDesign;
        }
        true
    }
}
