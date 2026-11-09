//! One-shot Design selection after checked DROP publication.
//! The removed native number is an inert preference, never a write capability.
use super::model::Model;
use crate::workspace::{DocumentView, ObjectKind, State, schema::Schema};

impl State {
    /// Move paid owner buffers only after operation-specific full-witness publication.
    /// Never call on a receipt, ACK, failed read or plain Database observation.
    pub(in crate::workspace) fn finish_field_drop_publication(&mut self) {
        let revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let selection = self.field_drop.draft.take().and_then(|mut draft| {
            let revision = revision?;
            if self.view != DocumentView::Design
                || revision == draft.target.tab.target.document_revision
            {
                return None;
            }

            draft.target.tab.target.document_revision = revision;
            if !self.objects.is_active_tab(&draft.target.tab) {
                return None;
            }

            Some(draft.target)
        });
        self.field_drop = Model {
            selection,
            ..Model::default()
        };
    }

    /// Resolve against fresh native identities, not the removed row's old position.
    pub(in crate::workspace) fn take_field_drop_schema_selection(
        &mut self,
        schema: &Schema,
    ) -> Option<usize> {
        let target = self.field_drop.selection.take()?;
        if self.view != DocumentView::Design
            || self.database.as_ref().is_none_or(|database| {
                database.document_revision != target.tab.target.document_revision
            })
            || !self.objects.is_active_tab(&target.tab)
            || !matches!(&target.tab.target.kind, ObjectKind::Table { name } if *name == schema.table)
        {
            return None;
        }

        let mut next = None;
        let mut previous = None;
        for (position, field) in schema.fields.iter().enumerate() {
            if field.number == target.native_number
                || schema.fields[..position]
                    .iter()
                    .any(|prior| prior.number == field.number)
            {
                self.error = Some(self.localizer.text("objects-restore-refused"));
                return None;
            }

            if field.number > target.native_number
                && next.is_none_or(|(_, number)| field.number < number)
            {
                next = Some((position, field.number));
            }
            if field.number < target.native_number
                && previous.is_none_or(|(_, number)| field.number > number)
            {
                previous = Some((position, field.number));
            }
        }

        let selected = next.or(previous).map(|(position, _)| position);
        if selected.is_none() {
            self.error = Some(self.localizer.text("objects-restore-refused"));
        }
        selected
    }
}
