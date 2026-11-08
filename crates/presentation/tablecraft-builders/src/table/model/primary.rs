//! Manual key choice is distinct from the protected Datasheet-generated ID.
use super::{
    DraftField, DraftFieldId, DraftFieldType, DraftNumberSize, FieldProperties, TableDraft,
};
use crate::table::{MAX_DRAFT_FIELDS, MAX_DRAFT_NAME_BYTES, TableDraftError};
use std::fmt::Write;

impl DraftField {
    pub fn supports_manual_primary_key(&self) -> bool {
        !self.protected && self.supports_primary_type(self.kind)
    }

    pub fn supports_primary_type(&self, kind: DraftFieldType) -> bool {
        match kind {
            DraftFieldType::Number => {
                self.properties.cached_number_size() == DraftNumberSize::LongInteger
            }
            DraftFieldType::LargeInteger | DraftFieldType::Currency => true,
            _ => false,
        }
    }
}

impl TableDraft {
    /// Choose one supported manual key at its actual field position, or keep a
    /// keyless manual design. No data, Required policy or field identity changes.
    pub fn set_primary_key(&mut self, field: Option<DraftFieldId>) -> Result<(), TableDraftError> {
        let selected = field
            .map(|id| {
                self.fields
                    .iter()
                    .position(|field| field.id == id)
                    .ok_or(TableDraftError::MissingField)
            })
            .transpose()?;
        let current = self.fields.iter().position(|field| field.primary);
        if selected == current {
            return Ok(());
        }
        if current.is_some_and(|position| self.fields[position].protected) {
            return Err(TableDraftError::ProtectedField);
        }
        if selected.is_some_and(|position| !self.fields[position].supports_manual_primary_key()) {
            return Err(TableDraftError::InvalidPrimaryKey);
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        for (position, field) in self.fields.iter_mut().enumerate() {
            field.primary = Some(position) == selected;
        }
        self.revision = revision;
        self.modified = true;
        Ok(())
    }

    /// Accept the explicit no-key prompt's automatic-ID choice. New identity and
    /// storage are prepared before changing any field. Collision naming is the
    /// bounded ID/ID1/ID2 policy, pending native Access comparison.
    pub fn add_incrementing_primary_key(&mut self) -> Result<DraftFieldId, TableDraftError> {
        if !self.design || self.fields.iter().any(|field| field.primary) {
            return Err(TableDraftError::ProtectedField);
        }
        if self.fields.len() >= MAX_DRAFT_FIELDS {
            return Err(TableDraftError::Limit);
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        let next_field = self
            .next_field
            .checked_add(1)
            .ok_or(TableDraftError::Limit)?;
        let mut name = String::new();
        name.try_reserve_exact(MAX_DRAFT_NAME_BYTES)
            .map_err(|_| TableDraftError::Allocation)?;
        for suffix in 0..=self.fields.len() {
            name.clear();
            name.push_str("ID");
            if suffix != 0 {
                write!(&mut name, "{suffix}").map_err(|_| TableDraftError::Allocation)?;
            }
            if !self
                .fields
                .iter()
                .any(|field| field.name.eq_ignore_ascii_case(&name))
            {
                break;
            }
        }
        self.fields
            .try_reserve_exact(1)
            .map_err(|_| TableDraftError::Allocation)?;
        let id = DraftFieldId(self.next_field);
        self.fields.insert(
            0,
            DraftField {
                id,
                name,
                kind: DraftFieldType::AutoNumberIncrement,
                primary: true,
                protected: true,
                description: None,
                properties: FieldProperties::new(DraftFieldType::AutoNumberIncrement),
            },
        );
        self.next_field = next_field;
        self.revision = revision;
        self.modified = true;
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
