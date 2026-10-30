//! Borrowed ORIGINAL and joint FINAL row projections for actual key replacements.
use super::SlotKeyUpdate;
use crate::table::indexes::key::KeyRow;
use crate::{EditError, InspectError, TableSchema, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::values::Value;

pub(in crate::table) struct RowKeyUpdate<'a> {
    pub reference: u32,
    pub columns: &'a [usize],
    pub before: &'a [Value],
    pub after: &'a [Value],
}

pub(in crate::table::indexes::mutation) trait KeyChange {
    fn reference(&self) -> u32;
    fn columns(&self) -> &[usize];
    fn original(&self, column: usize) -> Option<&Value>;
    fn final_value(&self, column: usize) -> Option<&Value>;
}

impl KeyChange for SlotKeyUpdate<'_> {
    fn reference(&self) -> u32 {
        self.reference
    }

    fn columns(&self) -> &[usize] {
        self.columns
    }

    fn original(&self, column: usize) -> Option<&Value> {
        self.columns
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.before.get(position))
    }

    fn final_value(&self, column: usize) -> Option<&Value> {
        // Preserve the actual single-slot consumer's override semantics without
        // modifying its ORIGINAL sibling values or allocating a replacement row.
        KeyRow::get(self, column)
    }
}

impl KeyChange for RowKeyUpdate<'_> {
    fn reference(&self) -> u32 {
        self.reference
    }

    fn columns(&self) -> &[usize] {
        self.columns
    }

    fn original(&self, column: usize) -> Option<&Value> {
        self.columns
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.before.get(position))
    }

    fn final_value(&self, column: usize) -> Option<&Value> {
        self.columns
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.after.get(position))
    }
}

pub(super) struct Original<'a, T: KeyChange + ?Sized>(pub &'a T);
pub(super) struct Final<'a, T: KeyChange + ?Sized>(pub &'a T);

impl<T: KeyChange + ?Sized> KeyRow for Original<'_, T> {
    fn get(&self, column: usize) -> Option<&Value> {
        self.0.original(column)
    }
}

impl<T: KeyChange + ?Sized> KeyRow for Final<'_, T> {
    fn get(&self, column: usize) -> Option<&Value> {
        self.0.final_value(column)
    }
}

/// Check the borrowed projection/identity inventory without adopting any staged
/// row bytes as authority. Actual original keys still match the source tree.
pub(in crate::table::indexes::mutation) fn validate(
    schema: &TableSchema,
    updates: &[RowKeyUpdate<'_>],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    for (position, update) in updates.iter().enumerate() {
        reader.tick()?;
        if update.columns.len() != update.before.len() || update.columns.len() != update.after.len()
        {
            return Err(EditError::Preservation);
        }
        reader.work(position)?;
        if updates[..position]
            .iter()
            .any(|previous| previous.reference == update.reference)
        {
            return Err(EditError::Preservation);
        }
        for (position, &column) in update.columns.iter().enumerate() {
            reader.tick()?;
            reader.work(position)?;
            if schema.columns().get(column).is_none()
                || update.columns[..position].contains(&column)
            {
                return Err(InspectError::Malformed.into());
            }
        }
    }
    Ok(())
}
