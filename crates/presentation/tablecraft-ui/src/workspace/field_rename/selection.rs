//! Inert native-field selection retained only after checked rename publication.
//! Move already admitted strings; this owns neither native handles nor save authority.
#[cfg(test)]
mod tests;

use super::{FieldRenameTarget, model::Model};
use crate::workspace::{DocumentView, ObjectKind, State, schema::Schema};

impl State {
    pub(super) fn finish_field_rename_presentation(&mut self) {
        let revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let selection = self.field_rename.draft.take().and_then(|mut draft| {
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
            // The accepted proposal and native number were matched to the actual
            // consuming runtime capability before full-image replacement. Reuse
            // these paid buffers only as a fresh-schema selection preference.
            draft.target.column = draft.proposed;
            Some(draft.target)
        });
        self.field_rename = Model {
            selection,
            ..Model::default()
        };
    }

    /// Consume a preference once, after schema admission and object binding.
    /// Resolve the native number and exact accepted name, never an old row index.
    pub(in crate::workspace) fn take_field_rename_schema_selection(
        &mut self,
        schema: &Schema,
    ) -> Option<usize> {
        let target = self.field_rename.selection.take()?;
        if self.view != DocumentView::Design
            || self.database.as_ref().is_none_or(|database| {
                database.document_revision != target.tab.target.document_revision
            })
            || !self.objects.is_active_tab(&target.tab)
            || !matches!(&target.tab.target.kind, ObjectKind::Table { name } if *name == schema.table)
        {
            return None;
        }
        let position = native_position(schema, &target);
        if position.is_none() {
            self.error = Some(self.localizer.text("objects-restore-refused"));
        }
        position
    }
}

fn native_position(schema: &Schema, target: &FieldRenameTarget) -> Option<usize> {
    let mut matching = schema
        .fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.number == target.native_number);
    let (position, field) = matching.next()?;
    (matching.next().is_none() && field.name == target.column).then_some(position)
}
