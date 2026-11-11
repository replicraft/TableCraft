//! Source-free field policies and exact unevaluated default input.
mod number;

pub use number::DraftNumberSize;

use super::{DraftField, DraftFieldId, DraftFieldType, TableDraft};
use crate::table::{
    MAX_DRAFT_DEFAULT_BYTES, MIN_SHORT_TEXT_UNITS, SHORT_TEXT_UNITS, TableDraftError,
};
use tablecraft_schema::DecimalConstraints;

const INITIAL_DECIMAL_PRECISION: u8 = 18;
const INITIAL_DECIMAL_SCALE: u8 = 0;

/// The initial declaration's default policy, separate from a pending row value.
/// Literal input is not parsed or converted by this renderer-neutral model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DraftDefaultMode {
    /// Use the current field type's supported creation default policy.
    #[default]
    TypeDefault,
    /// Explicitly omit a stored default declaration.
    Absent,
    /// Request an explicit NULL default, subject to native admission.
    Null,
    /// Exact unevaluated input, bounded by `MAX_DRAFT_DEFAULT_BYTES` on assignment.
    Literal(String),
}

impl DraftDefaultMode {
    pub(crate) fn retained_bytes(&self) -> usize {
        match self {
            Self::Literal(value) => value.capacity(),
            Self::TypeDefault | Self::Absent | Self::Null => 0,
        }
    }

    pub(crate) fn try_copy(&self) -> Result<Self, TableDraftError> {
        match self {
            Self::TypeDefault => Ok(Self::TypeDefault),
            Self::Absent => Ok(Self::Absent),
            Self::Null => Ok(Self::Null),
            Self::Literal(value) => {
                let mut copied = String::new();
                copied
                    .try_reserve_exact(value.len())
                    .map_err(|_| TableDraftError::Allocation)?;
                copied.push_str(value);
                Ok(Self::Literal(copied))
            }
        }
    }
}

pub(super) struct FieldProperties {
    text_units: u16,
    number_size: DraftNumberSize,
    decimal: DecimalConstraints,
    required: Option<bool>,
    allow_zero_length: Option<bool>,
    default_mode: DraftDefaultMode,
}

impl FieldProperties {
    pub(super) fn cached_number_size(&self) -> DraftNumberSize {
        self.number_size
    }

    pub(super) fn new(kind: DraftFieldType) -> Self {
        let ordinary = kind != DraftFieldType::AutoNumberIncrement;
        Self {
            text_units: SHORT_TEXT_UNITS,
            number_size: DraftNumberSize::LongInteger,
            decimal: DraftNumberSize::initial_decimal_constraints(),
            // Authoring follows Access UI defaults. The native API separately
            // preserves absent declarations and its programmatic defaults.
            required: ordinary.then_some(false),
            allow_zero_length: ordinary.then_some(true),
            default_mode: DraftDefaultMode::TypeDefault,
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.default_mode.retained_bytes()
    }
}

impl DraftField {
    /// Effective Short Text capacity in UTF-16 units.
    /// A cached capacity survives other field types.
    pub fn short_text_units(&self) -> Option<u16> {
        (self.kind == DraftFieldType::ShortText).then_some(self.properties.text_units)
    }

    /// An explicit declaration only; omission does not invent a native property.
    pub fn required(&self) -> Option<bool> {
        self.properties.required
    }

    /// Applicable only to Short Text; a hidden choice survives other field types.
    pub fn allow_zero_length(&self) -> Option<bool> {
        if self.kind == DraftFieldType::ShortText {
            self.properties.allow_zero_length
        } else {
            None
        }
    }

    pub fn default_mode(&self) -> &DraftDefaultMode {
        &self.properties.default_mode
    }
}

impl TableDraft {
    pub fn set_field_text_units(
        &mut self,
        id: DraftFieldId,
        units: u16,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if self.fields[position].kind != DraftFieldType::ShortText
            || !(MIN_SHORT_TEXT_UNITS..=SHORT_TEXT_UNITS).contains(&units)
        {
            return Err(TableDraftError::InvalidProperty);
        }
        if self.fields[position].properties.text_units == units {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].properties.text_units = units;
        self.finish_property_change(revision);
        Ok(())
    }

    pub fn set_field_required(
        &mut self,
        id: DraftFieldId,
        required: Option<bool>,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if self.fields[position].properties.required == required {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].properties.required = required;
        self.finish_property_change(revision);
        Ok(())
    }

    pub fn set_field_allow_zero_length(
        &mut self,
        id: DraftFieldId,
        allow: Option<bool>,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if self.fields[position].kind != DraftFieldType::ShortText {
            return Err(TableDraftError::InvalidProperty);
        }
        if self.fields[position].properties.allow_zero_length == allow {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;

        self.fields[position].properties.allow_zero_length = allow;
        self.finish_property_change(revision);
        Ok(())
    }

    pub fn set_field_default(
        &mut self,
        id: DraftFieldId,
        mode: DraftDefaultMode,
    ) -> Result<(), TableDraftError> {
        let position = self.ordinary_field_position(id)?;
        if let DraftDefaultMode::Literal(value) = &mode
            && value.len() > MAX_DRAFT_DEFAULT_BYTES
        {
            return Err(TableDraftError::Limit);
        }
        if self.fields[position].properties.default_mode == mode {
            return Ok(());
        }
        let revision = self.revision.checked_add(1).ok_or(TableDraftError::Limit)?;
        // Copy the bounded text rather than retaining an arbitrarily oversized
        // caller String allocation with a short visible value.
        let copied = mode.try_copy()?;

        self.fields[position].properties.default_mode = copied;
        self.finish_property_change(revision);
        Ok(())
    }

    fn ordinary_field_position(&self, id: DraftFieldId) -> Result<usize, TableDraftError> {
        let position = self
            .fields
            .iter()
            .position(|field| field.id == id)
            .ok_or(TableDraftError::MissingField)?;
        if self.fields[position].protected {
            return Err(TableDraftError::ProtectedField);
        }
        Ok(position)
    }

    fn finish_property_change(&mut self, revision: u64) {
        self.revision = revision;
        self.modified = true;
    }
}

#[cfg(test)]
mod tests;
