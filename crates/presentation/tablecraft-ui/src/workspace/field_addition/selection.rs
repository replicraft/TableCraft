//! One-shot Design focus after the operation's checked full-image publication.
#[cfg(test)]
mod tests;

use super::{Model, table_name};
use crate::workspace::{DocumentView, ObjectKind, ObjectTab, State, schema::Schema};

pub(super) struct Selection {
    tab: ObjectTab,
    column: String,
}

impl Selection {
    pub(super) fn heap_bytes(&self) -> Option<usize> {
        table_name(&self.tab)?
            .capacity()
            .checked_add(self.column.capacity())
    }
}

impl State {
    /// Called only after exact save matching and host-proven full-witness graph
    /// publication. Original/no-write acknowledgement must not advance revision.
    /// Moving paid draft buffers retains only an inert presentation preference.
    pub(super) fn finish_field_addition_presentation(&mut self) {
        let revision = self
            .database
            .as_ref()
            .map(|database| database.document_revision);
        let selection = self.field_addition.draft.take().and_then(|mut draft| {
            let revision = revision?;
            if self.view != DocumentView::Design || revision <= draft.tab.target.document_revision {
                return None;
            }

            draft.tab.target.document_revision = revision;
            if !self.objects.is_active_tab(&draft.tab) {
                return None;
            }

            Some(Selection {
                tab: draft.tab,
                column: draft.column,
            })
        });
        self.field_addition = Model {
            selection,
            ..Model::default()
        };
    }

    /// Resolve only after normal schema admission binds the active object.
    /// The actual unique native number comes from fresh metadata, never from a
    /// pending blank row, previous field count or prior visual position.
    pub(in crate::workspace) fn take_field_addition_schema_selection(
        &mut self,
        schema: &Schema,
    ) -> Option<usize> {
        let selection = self.field_addition.selection.take()?;
        if self.view != DocumentView::Design
            || self.database.as_ref().is_none_or(|database| {
                database.document_revision != selection.tab.target.document_revision
            })
            || !self.objects.is_active_tab(&selection.tab)
            || !matches!(&selection.tab.target.kind, ObjectKind::Table { name } if *name == schema.table)
        {
            return None;
        }

        let position = native_position(schema, &selection.column);
        if position.is_none() {
            self.error = Some(self.localizer.text("objects-restore-refused"));
        }
        position
    }
}

fn native_position(schema: &Schema, column: &str) -> Option<usize> {
    let mut selected = None;
    for (position, field) in schema.fields.iter().enumerate() {
        if schema.fields[..position]
            .iter()
            .any(|prior| prior.number == field.number)
        {
            return None;
        }
        if field.name == column && selected.replace((position, field.number)).is_some() {
            return None;
        }
    }
    selected.map(|(position, _native_number)| position)
}
