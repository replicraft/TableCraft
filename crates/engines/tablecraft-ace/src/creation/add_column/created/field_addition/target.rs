//! Closed typed declarations, without caller-supplied property bodies or flags.
use super::FieldAdditionError;
use crate::CreateColumn;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

static NUMBER_ZERO: Value = Value::Int32(0);

/// Explicit semantic declaration for the separate field-addition candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionTarget {
    /// Nullable ordinary LONG, Required(false), exact typed DefaultValue zero.
    NumberLongZero,
    /// Nullable Short Text, Required(false), AllowZeroLength(true), no default.
    ShortText {
        /// Explicit native capacity of 1 through 255 UTF-16 code units.
        max_utf16_units: usize,
        /// Explicit Unicode compression permission, without changing old defaults.
        unicode_compression: bool,
    },
}

impl FieldAdditionTarget {
    pub(super) fn declaration(self, name: &str) -> Result<CreateColumn<'_>, FieldAdditionError> {
        Ok(match self {
            Self::NumberLongZero => CreateColumn::new(name, ScalarType::Int32)
                .with_required(false)
                .with_default_literal(&NUMBER_ZERO)?,
            Self::ShortText {
                max_utf16_units,
                unicode_compression,
            } => CreateColumn::short_text(name, max_utf16_units)?
                .with_required(false)
                .with_allow_zero_length(true)?
                .with_unicode_compression(unicode_compression)?,
        })
    }

    pub(super) fn from_declaration(column: &CreateColumn<'_>) -> Result<Self, FieldAdditionError> {
        if column.required() != Some(false)
            || column.description().is_some()
            || column.primary_key_name().is_some()
            || column.incrementing_autonumber()
        {
            return Err(FieldAdditionError::Source);
        }
        match column.scalar_type() {
            ScalarType::Int32
                if column.default_literal() == Some(&NUMBER_ZERO)
                    && column.allow_zero_length().is_none()
                    && !column.unicode_compression() =>
            {
                Ok(Self::NumberLongZero)
            }
            ScalarType::Text
                if column.default_literal().is_none()
                    && column.allow_zero_length() == Some(true) =>
            {
                Ok(Self::ShortText {
                    max_utf16_units: column
                        .max_text_utf16_units()
                        .ok_or(FieldAdditionError::Source)?,
                    unicode_compression: column.unicode_compression(),
                })
            }
            _ => Err(FieldAdditionError::Source),
        }
    }
}
