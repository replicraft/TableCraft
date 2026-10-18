use super::{EditError, WriteBlocker, gates, physical};
use crate::{
    FileFormat, InspectError, NativeImage, check_table_constraints,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

/// One snapshot-local sequential row update. Ordinals are not durable row IDs.
/// No Debug implementation exposes the expected or replacement value.
pub struct Int32Update {
    row: usize,
    expected: i32,
    value: i32,
}
impl Int32Update {
    /// Address one row of the exact captured image, with an expected value.
    pub const fn new(row: usize, expected: i32, value: i32) -> Self {
        Self {
            row,
            expected,
            value,
        }
    }
    /// Sequential live-row ordinal in this captured image only.
    pub const fn row(&self) -> usize {
        self.row
    }
    /// Expected original Int32, checked before preparing edits.
    pub const fn expected(&self) -> i32 {
        self.expected
    }
    /// Replacement Int32 with unchanged carrier/width/nullness.
    pub const fn value(&self) -> i32 {
        self.value
    }
}

/// Complete before/after images with changes confined to validated field bytes.
/// Planning never writes to a source. Persistence requires a qualified journal
/// and exclusive offline access; `after_bytes` alone supplies no durability.
pub struct NativeEditPlan {
    before: Vec<u8>,
    after: Vec<u8>,
    changed_rows: usize,
}
impl NativeEditPlan {
    /// Exact captured original image.
    pub fn before_bytes(&self) -> &[u8] {
        &self.before
    }
    /// Candidate image, with every byte outside its fixed field patches equal.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Rows whose value changed; matched no-op assignments are excluded.
    pub const fn changed_rows(&self) -> usize {
        self.changed_rows
    }
}

impl NativeImage {
    /// Plan bounded existing non-NULL, non-indexed fixed Int32 replacements in an
    /// unflagged Jet 4 table. Reject AutoNumber, unqualified constraints/properties,
    /// relocated rows and all ACE writes (data-macro detection is unqualified).
    /// Complete before/after field/relationship checks must pass. No defaults,
    /// VBA, application callbacks or cascades execute. Failure publishes no plan.
    pub fn plan_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: &[Int32Update],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        let _span = tracing::debug_span!("native.edit.plan", assignments = updates.len()).entered();
        check(budget, cancelled)?;
        if self.catalog().header().format() != FileFormat::Jet4 {
            return Err(EditError::Unsupported(WriteBlocker::Format));
        }
        if updates.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        for (position, update) in updates.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if updates[..position]
                .iter()
                .any(|previous| previous.row() == update.row())
            {
                return Err(EditError::Conflict);
            }
        }
        let structural = gates::before(&self, table, budget, cancelled)?;
        let schema = structural.schema();
        let position = schema
            .columns
            .iter()
            .position(|candidate| candidate.name().eq_ignore_ascii_case(column))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let selected = &schema.columns[position];
        if selected.scalar_type() != Some(ScalarType::Int32)
            || selected.field.flags != (FIXED_FLAG | UPDATABLE_FLAG)
            || selected.field.extended_flags != 0
            || !selected.fixed()
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let range = selected.field.offset..selected.field.offset + size_of::<i32>();
        if schema.columns.iter().enumerate().any(|(other, candidate)| {
            other != position
                && candidate.fixed()
                && candidate.field.length != 0
                && candidate.field.offset < range.end
                && candidate.field.offset + candidate.field.length > range.start
        }) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if structural.indexes().iter().any(|index| {
            index
                .index()
                .columns()
                .iter()
                .any(|key| key.number() == selected.number())
        }) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let scope = self.catalog().catalog().scope();
        let mut source = self.reader();
        let patches = physical::patches(
            &mut Reader {
                source: &mut source,
                header: self.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            },
            schema,
            position,
            updates,
        )?;
        let length = self.bytes.len();
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        // Copy with bounded cancellation latency, preserving all unknown bytes.
        for chunk in self.bytes.chunks(crate::preservation::COPY_CHUNK_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            after.extend_from_slice(chunk);
        }
        for patch in &patches {
            check(budget, cancelled)?;
            let target = after
                .get_mut(patch.offset..patch.offset + size_of::<i32>())
                .ok_or(EditError::Preservation)?;
            if target != patch.before {
                return Err(EditError::Preservation);
            }
            target.copy_from_slice(&patch.after);
        }
        // An independent linear comparison proves that opaque content outside
        // the patch list is byte-identical, including unused pages and padding.
        let mut previous = 0;
        for patch in &patches {
            equal(
                &self.bytes[previous..patch.offset],
                &after[previous..patch.offset],
                budget,
                cancelled,
            )?;
            previous = patch.offset + size_of::<i32>();
        }
        equal(
            &self.bytes[previous..],
            &after[previous..],
            budget,
            cancelled,
        )?;
        gates::qualified(&check_table_constraints(
            &mut Cursor::new(&after),
            scope,
            table,
            budget,
            cancelled,
        )?)?;
        check(budget, cancelled)?;
        Ok(NativeEditPlan {
            before: self.bytes,
            after,
            changed_rows: patches.len(),
        })
    }
}

fn equal(
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for (left, right) in before
        .chunks(crate::preservation::COPY_CHUNK_BYTES)
        .zip(after.chunks(crate::preservation::COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, left.len() as u64)
            .map_err(InspectError::from)?;
        if left != right {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}
