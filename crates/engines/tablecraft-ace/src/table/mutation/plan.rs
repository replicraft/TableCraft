use super::super::indexes::{Int32KeyUpdate, index_patches};
use super::{EditError, WriteBlocker, field, gates, physical, rows};
use crate::{FileFormat, InspectError, NativeImage, format::reader::Reader, header::check};
use tablecraft_core::limits::ResourceBudget;
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
    pub(super) before: Vec<u8>,
    pub(super) after: Vec<u8>,
    pub(super) changed_rows: usize,
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
        rows::unique(updates, budget, cancelled)?;
        let structural = gates::before(&self, table, budget, cancelled)?;
        let schema = structural.schema();
        let position = field::fixed_column(schema, column, kind, WIDTH)?;
        let selected = &schema.columns[position];
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
        self.finish_updates(
            table,
            &patches,
            &index_patches,
            patches.len(),
            budget,
            cancelled,
        )
    }
}
