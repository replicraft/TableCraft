//! Exact optional native field descriptions, independent of key protection.
use super::{DraftFieldId, TableDraft};
use crate::table::{MAX_DRAFT_DESCRIPTION_BYTES, MAX_DRAFT_DESCRIPTION_UNITS, TableDraftError};

impl TableDraft {
    /// Preserve absence, present-empty text and exact Unicode/whitespace.
    /// Allocation, missing-field, length and revision refusals leave the draft intact.
    pub fn set_field_description(
        &mut self,
        id: DraftFieldId,
        description: Option<&str>,
    ) -> Result<(), TableDraftError> {
        let position = self
            .fields
            .iter()
            .position(|field| field.id == id)
            .ok_or(TableDraftError::MissingField)?;
        if description.is_some_and(|text| {
            text.len() > MAX_DRAFT_DESCRIPTION_BYTES
                || text
                    .encode_utf16()
                    .take(MAX_DRAFT_DESCRIPTION_UNITS + 1)
                    .count()
                    > MAX_DRAFT_DESCRIPTION_UNITS
        }) {
            return Err(TableDraftError::InvalidDescription);
        }
        if self.fields[position].description.as_deref() == description {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        let owned = description
            .map(|text| {
                let mut owned = String::new();
                owned
                    .try_reserve_exact(text.len())
                    .map_err(|_| TableDraftError::Allocation)?;
                owned.push_str(text);
                Ok::<_, TableDraftError>(owned)
            })
            .transpose()?;
        self.fields[position].description = owned;
        self.revision = revision;
        self.modified = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
