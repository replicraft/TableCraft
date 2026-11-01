//! Borrowed original fixed and variable assignments, without native authority.
use super::{FixedScalarUpdate, SlotScalarUpdate};
use tablecraft_core::values::Value;

/// One selected fitted-record column, with explicit original-value expectations.
pub enum FittedColumnUpdates<'a> {
    /// Existing fixed scalar payload/presence changes.
    Fixed {
        /// Exact native column name.
        column: &'a str,
        /// Complete original-row fixed replacements.
        updates: &'a [FixedScalarUpdate],
    },
    /// One ordinary Short Text/Binary field using original record capacity.
    Variable {
        /// Exact native column name.
        column: &'a str,
        /// Complete original-row variable replacements.
        updates: &'a [SlotScalarUpdate],
    },
}

pub(in crate::table::mutation) struct FittedCell<'a> {
    pub row: usize,
    pub expected: &'a Value,
    pub value: &'a Value,
}

impl FittedColumnUpdates<'_> {
    pub(in crate::table::mutation) fn name(&self) -> &str {
        match self {
            Self::Fixed { column, .. } | Self::Variable { column, .. } => column,
        }
    }

    pub(in crate::table::mutation) fn len(&self) -> usize {
        match self {
            Self::Fixed { updates, .. } => updates.len(),
            Self::Variable { updates, .. } => updates.len(),
        }
    }

    pub(in crate::table::mutation) fn cell(&self, position: usize) -> Option<FittedCell<'_>> {
        match self {
            Self::Fixed { updates, .. } => updates.get(position).map(|cell| FittedCell {
                row: cell.row(),
                expected: cell.expected(),
                value: cell.value(),
            }),
            Self::Variable { updates, .. } => updates.get(position).map(|cell| FittedCell {
                row: cell.row(),
                expected: cell.expected(),
                value: cell.value(),
            }),
        }
    }
}
