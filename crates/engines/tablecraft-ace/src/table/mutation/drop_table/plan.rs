use super::{NativeTableDropIdentity, SourceProfile, TableDropError, source, verify};
use crate::{EditError, InspectError, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

/// Completed readonly eligibility for one exact captured table identity.
/// It owns no candidate image, filesystem lease or persistence permission.
pub struct NativeTableDropInspection {
    table: String,
    identity: NativeTableDropIdentity,
    declared_rows: u32,
    profile: SourceProfile,
}

impl NativeTableDropInspection {
    fn from_source(source: source::Qualified) -> Self {
        Self {
            table: source.table,
            identity: source.identity,
            declared_rows: source.declared_rows,
            profile: source.profile,
        }
    }

    /// Actual original native spelling, independent of lookup spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Original caller-scoped and native physical catalog identities.
    pub const fn identity(&self) -> NativeTableDropIdentity {
        self.identity
    }

    /// Fully qualified stored user-table live-record count before deletion.
    pub const fn declared_rows(&self) -> u32 {
        self.declared_rows
    }
}

/// An unpublished logical table deletion with complete preserved native images.
///
/// The empty table's catalog root becomes a tombstone and MSysObjects' live count
/// decreases. Table definitions, records, indexes, maps and historical bytes stay
/// allocated and byte-exact; this plan does not reclaim or shred storage. The
/// closed source forbids permissions, relationships and application dependencies
/// requiring additional catalog edits. Persistence requires separately retained
/// original-request reconstruction and journal ownership.
pub struct NativeTableDropPlan {
    before: NativeImage,
    after: Vec<u8>,
    inspection: NativeTableDropInspection,
}

impl NativeTableDropPlan {
    /// Exact original table spelling; an absent target cannot produce a plan.
    pub fn table(&self) -> &str {
        self.inspection.table()
    }

    /// Original persistent and caller-scoped identities.
    pub const fn identity(&self) -> NativeTableDropIdentity {
        self.inspection.identity()
    }

    /// Stored user-table live-record count before logical deletion.
    pub const fn declared_rows(&self) -> u32 {
        self.inspection.declared_rows()
    }

    /// Complete retained original, including every historical table page.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete unpublished candidate with retained allocated table storage.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    pub(crate) const fn source_profile(&self) -> SourceProfile {
        self.inspection.profile
    }
}

impl NativeImage {
    /// Inspect logical DROP eligibility without building an AFTER or acquiring a lease.
    ///
    /// # Errors
    /// Refuses missing targets, unsupported storage, active dependencies, cancellation,
    /// resource exhaustion and any unqualified source role or Name AutoCorrect state.
    pub fn inspect_table_drop_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableDropInspection, TableDropError> {
        let _span = tracing::debug_span!("native.table.drop.inspect").entered();
        let profile = source::select_profile(self, budget, cancelled)?;
        let original = source::qualify(self, table, profile, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(NativeTableDropInspection::from_source(original))
    }

    /// Logically delete one qualified empty table while retaining historical storage.
    ///
    /// Actual property framing selects the source profile exactly once; a failed
    /// source is not retried as another profile. Only the selected catalog slot
    /// flags and its table's live count change. No object body is executed.
    ///
    /// # Errors
    /// Refuses missing targets, unsupported source/dependencies, invalid catalog
    /// framing, cancellation, resource/allocation failure or preservation mismatch.
    pub fn plan_table_drop(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableDropPlan, TableDropError> {
        let profile = source::select_profile(&self, budget, cancelled)?;
        self.plan_table_drop_for_profile(table, profile, budget, cancelled)
    }

    pub(crate) fn plan_table_drop_for_profile(
        self,
        table: &str,
        profile: SourceProfile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableDropPlan, TableDropError> {
        let _span = tracing::debug_span!("native.table.drop.plan").entered();
        let original = source::qualify(&self, table, profile, budget, cancelled)?;
        let mut after =
            super::super::record_serialization::copy(self.as_bytes(), budget, cancelled)
                .map_err(serialization_error)?;
        verify::write(&self, &mut after, &original, budget, cancelled)?;
        verify::candidate(&self, &after, &original, budget, cancelled)?;
        check(budget, cancelled)?;

        Ok(NativeTableDropPlan {
            before: self,
            after,
            inspection: NativeTableDropInspection::from_source(original),
        })
    }
}

fn serialization_error(error: EditError) -> TableDropError {
    match error {
        EditError::Inspect(error) => TableDropError::Inspect(error),
        EditError::Preservation => TableDropError::Preservation,
        _ => TableDropError::Source,
    }
}

pub(super) fn offset(page: u32) -> Result<usize, TableDropError> {
    usize::try_from(page)
        .ok()
        .and_then(|page| page.checked_mul(crate::format::page::PAGE_BYTES))
        .ok_or_else(|| InspectError::StructureLimit.into())
}
