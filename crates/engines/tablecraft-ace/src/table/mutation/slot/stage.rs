//! Checked live-row spans and exact expected-value preconditions.
use super::super::{EditError, WriteBlocker, patch::BytePatch, rows};
use super::{SlotScalarUpdate, storage};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::{reader::Reader, row::MASK_BITS},
    header::check,
    table::{indexes::SlotKeyUpdate, record, record_layout},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MAX_PATCHES_PER_ROW: usize = 2;

pub(in crate::table::mutation) struct Patch {
    offset: usize,
    before: Vec<u8>,
    after: Vec<u8>,
}

impl BytePatch for Patch {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before
    }
    fn after(&self) -> &[u8] {
        &self.after
    }
    fn requires_structural_check(&self) -> bool {
        true
    }
}

pub(super) struct Changed<'a> {
    reference: u32,
    before: Vec<Value>,
    value: &'a Value,
}

pub(in crate::table::mutation) struct Staged<'a> {
    pub patches: Vec<Patch>,
    pub(super) changed: Vec<Changed<'a>>,
    columns: Vec<usize>,
}

impl Staged<'_> {
    pub fn changed_rows(&self) -> usize {
        self.changed.len()
    }

    pub fn keys(
        &self,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<SlotKeyUpdate<'_>>, EditError> {
        budget
            .charge_allocation(
                self.changed.len() as u64,
                size_of::<SlotKeyUpdate<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(self.changed.len())
            .map_err(|_| InspectError::Allocation)?;
        for changed in &self.changed {
            check(budget, cancelled)?;
            keys.push(SlotKeyUpdate {
                reference: changed.reference,
                columns: &self.columns,
                before: &changed.before,
                column,
                value: changed.value,
            });
        }
        Ok(keys)
    }
}

pub(in crate::table::mutation) fn stage<'a>(
    image: &NativeImage,
    structural: &TableCheck,
    column: usize,
    updates: &'a [SlotScalarUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<'a>, EditError> {
    let schema = structural.schema();
    let field = &schema.columns[column];
    for update in updates {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (field.storage_length() * 2) as u64)
            .map_err(InspectError::from)?;
        storage::validate(field, update.expected())?;
        storage::validate(field, update.value())?;
    }
    rows::unique(updates, budget, cancelled)?;

    budget
        .charge_allocation(schema.columns.len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(schema.columns.len())
        .map_err(|_| InspectError::Allocation)?;
    // Decode only the selected field and siblings in affected indexes. Opaque
    // Memo/OLE payloads elsewhere retain their original bytes without copying.
    for (position, candidate) in schema.columns.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (structural.indexes().len() * schema.columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if position == column
            || structural.indexes().iter().any(|index| {
                let keys = index.index().columns();
                keys.iter().any(|key| key.number() == field.number())
                    && keys.iter().any(|key| key.number() == candidate.number())
            })
        {
            columns.push(position);
        }
    }
    let selected = columns
        .iter()
        .position(|position| *position == column)
        .ok_or(InspectError::Malformed)?;
    let capacity = updates
        .len()
        .checked_mul(MAX_PATCHES_PER_ROW)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Patch>() as u64)
        .map_err(InspectError::from)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<Changed<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut source = image.reader();
    rows::visit_with_budget(
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        },
        schema,
        updates,
        |budget, row, update| {
            let layout = record_layout::parse(row.data, schema, budget)?;
            if layout.count != schema.columns.len() || layout.mask_start != row.mask {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let start = *layout
                .offsets
                .get(field.field.variable)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            let end = *layout
                .offsets
                .get(field.field.variable + 1)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            let mut decode_source = image.reader();
            let before = record::decode_selected(
                row.data,
                schema,
                Some(&columns),
                &mut Reader {
                    source: &mut decode_source,
                    header: image.catalog().header(),
                    budget,
                    cancelled,
                    reads: 0,
                },
            )?;
            if before.get(selected) != Some(update.expected()) {
                return Err(EditError::Conflict);
            }
            if update.expected() == update.value() {
                return Ok(());
            }

            // NULL preserves its complete old slot, including opaque residual
            // bytes. A present replacement must fit the actual slot exactly.
            if !matches!(update.value(), Value::Null) {
                let after = storage::encode_for_span(
                    field,
                    update.value(),
                    end - start,
                    budget,
                    cancelled,
                )?;
                if after.len() != end - start {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
                let stored = row.data.get(start..end).ok_or(InspectError::Malformed)?;
                if stored != after {
                    patches.push(Patch {
                        offset: row.offset + start,
                        before: copy(stored, budget)?,
                        after,
                    });
                }
            }
            let mask_offset = layout.mask_start + field.number() / MASK_BITS;
            let old_mask = *row.data.get(mask_offset).ok_or(InspectError::Malformed)?;
            let bit = 1_u8 << (field.number() % MASK_BITS);
            let new_mask = (old_mask & !bit)
                | if matches!(update.value(), Value::Null) {
                    0
                } else {
                    bit
                };
            if old_mask != new_mask {
                patches.push(Patch {
                    offset: row.offset + mask_offset,
                    before: copy(&[old_mask], budget)?,
                    after: copy(&[new_mask], budget)?,
                });
            }
            changed.push(Changed {
                reference: row.reference,
                before,
                value: update.value(),
            });
            Ok(())
        },
    )?;
    Ok(Staged {
        patches,
        changed,
        columns,
    })
}

fn copy(bytes: &[u8], budget: &mut ResourceBudget) -> Result<Vec<u8>, InspectError> {
    budget.charge_allocation(bytes.len() as u64, 1)?;
    budget.charge(Resource::Steps, bytes.len() as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(bytes);
    Ok(result)
}
