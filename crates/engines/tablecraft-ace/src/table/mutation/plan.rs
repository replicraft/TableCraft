use super::super::indexes::{Int32KeyUpdate, index_patches};
use super::{EditError, WriteBlocker, gates, physical, preservation};
use crate::{
    FileFormat, IndexCheckStatus, InspectError, NativeImage, check_table, check_table_constraints,
    format::{
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        reader::Reader,
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use std::{io::Cursor, ops::Range};
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

/// Complete before/after images confined to validated fields/index-entry spans.
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
    /// Candidate image, with all bytes outside its field/index-entry spans equal.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Rows whose value changed; matched no-op assignments are excluded.
    pub const fn changed_rows(&self) -> usize {
        self.changed_rows
    }
}

impl NativeImage {
    /// Plan bounded existing non-NULL fixed Int32 replacements in an
    /// unflagged Jet 4 table. Reject AutoNumber, unqualified constraints/properties,
    /// relocated rows and all ACE writes (data-macro detection is unqualified).
    /// Complete before/after field/relationship checks must pass. No defaults,
    /// VBA, application callbacks or cascades execute. Failure publishes no plan.
    /// Affected indexes must be unique Int32 trees whose existing topology,
    /// entry counts, prefixes and widths can be retained; broader changes fail.
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
        self.plan_int32_updates_inner(table, column, updates, budget, cancelled)
    }

    // Shared physical machinery; public entry points retain their own format
    // gates and return types. ACE previews must never become journalable plans.
    pub(super) fn plan_int32_updates_inner(
        self,
        table: &str,
        column: &str,
        updates: &[Int32Update],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        self.plan_fixed_updates_inner(table, column, updates, ScalarType::Int32, budget, cancelled)
    }

    pub(super) fn plan_fixed_updates_inner<
        U: physical::FixedAssignment<WIDTH>,
        const WIDTH: usize,
    >(
        self,
        table: &str,
        column: &str,
        updates: &[U],
        kind: ScalarType,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
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
        if selected.scalar_type() != Some(kind)
            || selected.field.length != WIDTH
            || selected.field.flags != (FIXED_FLAG | UPDATABLE_FLAG)
            || selected.field.extended_flags != 0
            || !selected.fixed()
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let range = selected.field.offset..selected.field.offset + WIDTH;
        if schema.columns.iter().enumerate().any(|(other, candidate)| {
            other != position
                && candidate.fixed()
                && candidate.field.length != 0
                && candidate.field.offset < range.end
                && candidate.field.offset + candidate.field.length > range.start
        }) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
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
        let indexed = structural.indexes().iter().any(|index| {
            index
                .index()
                .columns()
                .iter()
                .any(|key| key.number() == selected.number())
        });
        let mut keys = Vec::new();
        if indexed {
            if kind != ScalarType::Int32 || WIDTH != size_of::<i32>() {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            budget
                .charge_allocation(patches.len() as u64, size_of::<Int32KeyUpdate>() as u64)
                .map_err(InspectError::from)?;
            keys.try_reserve_exact(patches.len())
                .map_err(|_| InspectError::Allocation)?;
            for patch in &patches {
                check(budget, cancelled)?;
                keys.push(Int32KeyUpdate {
                    reference: patch.reference,
                    expected: i32::from_le_bytes(
                        patch
                            .before
                            .as_slice()
                            .try_into()
                            .map_err(|_| InspectError::Malformed)?,
                    ),
                    value: i32::from_le_bytes(
                        patch
                            .after
                            .as_slice()
                            .try_into()
                            .map_err(|_| InspectError::Malformed)?,
                    ),
                });
            }
        }
        let index_patches = index_patches(
            &self,
            schema,
            structural.indexes(),
            position,
            &keys,
            budget,
            cancelled,
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
                .get_mut(patch.offset..patch.offset + WIDTH)
                .ok_or(EditError::Preservation)?;
            if target != patch.before {
                return Err(EditError::Preservation);
            }
            target.copy_from_slice(&patch.after);
        }
        for patch in &index_patches {
            check(budget, cancelled)?;
            if patch.before.len() != patch.after.len() {
                return Err(EditError::Preservation);
            }
            let target = after
                .get_mut(patch.offset..patch.offset + patch.before.len())
                .ok_or(EditError::Preservation)?;
            if target != patch.before {
                return Err(EditError::Preservation);
            }
            target.copy_from_slice(&patch.after);
        }
        // An independent linear comparison proves that opaque content outside
        // the patch list is byte-identical, including unused pages and padding.
        let count = patches.len() + index_patches.len();
        budget
            .charge_allocation(count as u64, size_of::<Range<usize>>() as u64)
            .map_err(InspectError::from)?;
        let mut spans = Vec::new();
        spans
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for patch in &patches {
            spans.push(patch.offset..patch.offset + WIDTH);
        }
        for patch in &index_patches {
            spans.push(patch.offset..patch.offset + patch.before.len());
        }
        preservation::prove(&self.bytes, &after, spans, budget, cancelled)?;
        if !index_patches.is_empty()
            && check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?
                .indexes()
                .iter()
                .any(|index| index.status() != IndexCheckStatus::Checked)
        {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
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
