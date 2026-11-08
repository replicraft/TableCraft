//! Number declarations retain their selected storage size independently of raw input.
use super::{DraftField, DraftFieldId, DraftFieldType, TableDraft, TableDraftError};
use tablecraft_schema::DecimalConstraints;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DraftNumberSize {
    Byte,
    Integer,
    #[default]
    LongInteger,
    Single,
    Double,
    Decimal,
    ReplicationId,
}

impl DraftField {
    /// Effective Number size. Other kinds retain the hidden choice unchanged.
    pub fn number_size(&self) -> Option<DraftNumberSize> {
        (self.kind == DraftFieldType::Number).then_some(self.properties.number_size)
    }

    /// Effective Decimal precision and scale, without conversion of pending input.
    pub fn decimal_constraints(&self) -> Option<DecimalConstraints> {
        (self.number_size() == Some(DraftNumberSize::Decimal)).then_some(self.properties.decimal)
    }
}

impl TableDraft {
    pub fn set_field_number_size(
        &mut self,
        id: DraftFieldId,
        size: DraftNumberSize,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if self.fields[position].kind != DraftFieldType::Number {
            return Err(TableDraftError::InvalidProperty);
        }
        if self.fields[position].properties.number_size == size {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].properties.number_size = size;
        self.finish_property_change(revision);
        Ok(())
    }

    pub fn set_field_decimal_constraints(
        &mut self,
        id: DraftFieldId,
        constraints: DecimalConstraints,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if self.fields[position].number_size() != Some(DraftNumberSize::Decimal) {
            return Err(TableDraftError::InvalidProperty);
        }
        if self.fields[position].properties.decimal == constraints {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].properties.decimal = constraints;
        self.finish_property_change(revision);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
