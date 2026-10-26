//! Key-only output with original logical positions for constraint evaluation.
use super::super::{NativeIndex, key::KeyRow};
use crate::{InspectError, TableSchema, format::layout::INDEX_KEY_COLUMNS};
use arrayvec::ArrayVec;
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) struct CheckProjection<'a> {
    names: ArrayVec<&'a str, INDEX_KEY_COLUMNS>,
    positions: ArrayVec<usize, INDEX_KEY_COLUMNS>,
}

impl<'a> CheckProjection<'a> {
    pub fn new(
        schema: &'a TableSchema,
        index: &NativeIndex,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let mut names = ArrayVec::new();
        let mut positions = ArrayVec::new();

        for (position, column) in schema.columns().iter().enumerate() {
            super::tick(budget, cancelled)?;
            for key in index.columns() {
                super::tick(budget, cancelled)?;
                if key.number() == column.number() {
                    names
                        .try_push(column.name())
                        .map_err(|_| InspectError::Malformed)?;
                    positions
                        .try_push(position)
                        .map_err(|_| InspectError::Malformed)?;
                    break;
                }
            }
        }
        if names.len() != index.columns().len() {
            return Err(InspectError::Malformed);
        }

        Ok(Self { names, positions })
    }

    pub fn names(&self) -> &[&'a str] {
        &self.names
    }

    /// The sequential snapshot has already validated every actual carrier.
    /// Only these key columns determine omission; unrelated values never do.
    pub fn null_facts(
        &self,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(bool, bool), InspectError> {
        let mut all_null = true;
        let mut any_null = false;
        for &position in &self.positions {
            super::tick(budget, cancelled)?;
            let null = matches!(
                values.get(position).ok_or(InspectError::Malformed)?,
                Value::Null
            );
            all_null &= null;
            any_null |= null;
        }
        Ok((all_null, any_null))
    }

    pub fn row<'b>(&'b self, values: &'b [Value]) -> ProjectedKeyRow<'b> {
        ProjectedKeyRow {
            positions: &self.positions,
            values,
        }
    }
}

pub(super) struct ProjectedKeyRow<'a> {
    positions: &'a [usize],
    values: &'a [Value],
}

impl KeyRow for ProjectedKeyRow<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        let dense = self.positions.binary_search(&column).ok()?;
        self.values.get(dense)
    }
}
