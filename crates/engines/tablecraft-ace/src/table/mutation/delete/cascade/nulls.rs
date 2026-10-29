//! Source-bound surviving-cell changes, preserving old fixed payload on NULL.
use super::{EditError, reserve, state::Table};
use crate::{
    InspectError, NativeImage, WriteBlocker,
    format::data_type::NUMERIC_BYTES,
    header::check,
    table::{
        indexes::IndexKeyUpdate,
        mutation::{
            fixed::{FixedScalarUpdate, masked},
            patch::spans,
        },
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Guid, Value},
};
use tablecraft_schema::ScalarType;

pub(super) fn stage(
    image: &NativeImage,
    table: &Table,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<spans::Patch>, Vec<IndexKeyUpdate>), EditError> {
    let mut fields = Vec::new();
    let mut keys = Vec::new();
    for (column, native) in table.before.schema().columns().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, table.nulls.len() as u64)
            .map_err(InspectError::from)?;
        if !table.nulls.iter().any(|&(_, target)| target == column) {
            continue;
        }
        let mut updates = Vec::new();
        for &(row, target) in &table.nulls {
            check(budget, cancelled)?;
            if target != column {
                continue;
            }
            if table.selected.get(row).copied() != Some(false) {
                return Err(EditError::Preservation);
            }
            let value = table.original_value(row, column, budget)?;
            let expected = match value {
                Value::Byte(_)
                | Value::Int16(_)
                | Value::Int32(_)
                | Value::Int64(_)
                | Value::Currency(_)
                | Value::Guid(_)
                | Value::Decimal(_) => value.clone(),
                _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
            };
            reserve(&mut updates, budget)?;
            updates.push(FixedScalarUpdate::new(row, expected, Value::Null));
        }
        let kind = native
            .scalar_type()
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let mut output = Output {
            image,
            table,
            fields: &mut fields,
            keys: &mut keys,
            budget,
            cancelled,
        };
        match kind {
            ScalarType::Byte => output.add::<{ size_of::<u8>() }>(native.name(), &updates, kind)?,
            ScalarType::Int16 => {
                output.add::<{ size_of::<i16>() }>(native.name(), &updates, kind)?
            }
            ScalarType::Int32 => {
                output.add::<{ size_of::<i32>() }>(native.name(), &updates, kind)?
            }
            ScalarType::Int64 | ScalarType::Currency => {
                output.add::<{ size_of::<i64>() }>(native.name(), &updates, kind)?
            }
            ScalarType::Guid => output.add::<{ Guid::BYTE_LEN }>(native.name(), &updates, kind)?,
            ScalarType::Decimal => output.add::<NUMERIC_BYTES>(native.name(), &updates, kind)?,
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        }
    }
    Ok((fields, keys))
}

struct Output<'a> {
    image: &'a NativeImage,
    table: &'a Table,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    fields: &'a mut Vec<spans::Patch>,
    keys: &'a mut Vec<IndexKeyUpdate>,
}

impl Output<'_> {
    fn add<const WIDTH: usize>(
        &mut self,
        column: &str,
        updates: &[FixedScalarUpdate],
        kind: ScalarType,
    ) -> Result<(), EditError> {
        let staged = masked::stage::<WIDTH, false>(
            self.image,
            &self.table.before,
            column,
            updates,
            kind,
            self.budget,
            self.cancelled,
        )?;
        if staged.changed.len() != updates.len() {
            return Err(EditError::Preservation);
        }
        for field in &staged.patches {
            spans::push(self.fields, field, self.budget, self.cancelled)?;
        }
        for key in staged.keys {
            check(self.budget, self.cancelled)?;
            reserve(self.keys, self.budget)?;
            self.keys.push(key);
        }
        Ok(())
    }
}
