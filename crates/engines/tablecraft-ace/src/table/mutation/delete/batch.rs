//! Explicit multi-table final-image candidates; no action expansion or early output.
use super::{Ace16DeletePreview, TableRowDeletes, cascade, patches, relationships};
use crate::{
    EditError, InspectError, NativeImage, TableCheck,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::{MAX_COLUMNS, MAX_LIVE_ROWS},
    table::{
        indexes::{
            DeleteIndexPlan, IndexDataPatch, RetirementTable, audit_global, audit_retirements,
            audit_siblings, compose_retirements, index_delete_patches,
        },
        mutation::{gates, preview, rows},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) enum Admission {
    Restrict,
    Cascading(cascade::Proof),
}

struct Table {
    before: TableCheck,
    references: Vec<u32>,
    indexes: DeleteIndexPlan,
}

impl NativeImage {
    /// Maximum explicitly named table groups in a deletion candidate.
    pub const MAX_DELETE_BATCH_TABLES: usize = MAX_COLUMNS;
    /// Maximum cumulative requested rows across all deletion table groups.
    pub const MAX_DELETE_BATCH_ROWS: usize = MAX_LIVE_ROWS;

    /// Prepare one final-image deletion candidate across explicitly named tables.
    ///
    /// Every ordinal and expected raw record belongs to the original image.
    /// Duplicate table groups and duplicate/stale rows refuse. Named empty groups
    /// retain complete table/profile/relationship gates. All record and index
    /// edits are staged before one complete final-image membership check, so
    /// child and parent removals can be requested together without an intermediate
    /// broken image. This is an explicit physical batch, not SQL RESTRICT timing.
    ///
    /// Existing RESTRICT-only cross-table admission remains in force. Cascades,
    /// SET NULL, self relationships, data macros and normal ACE saves are not
    /// executed or qualified. Shared bitmap changes are composed once from the
    /// original image; exact global and per-table ownership/sibling proofs must
    /// complete before returning the non-journalable candidate.
    pub fn preview_ace16_batch_deletes(
        self,
        groups: &[TableRowDeletes<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16DeletePreview, EditError> {
        run(self, groups, Admission::Restrict, budget, cancelled)
    }
}

pub(super) fn run(
    image: NativeImage,
    groups: &[TableRowDeletes<'_>],
    admission: Admission,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16DeletePreview, EditError> {
    let _span =
        tracing::debug_span!("native.delete.batch_preview", groups = groups.len()).entered();
    let mut proven = match admission {
        Admission::Restrict => None,
        Admission::Cascading(proof) => Some(proof.into_checks().into_iter()),
    };
    check(budget, cancelled)?;
    preview::qualify(&image)?;
    if groups.len() > NativeImage::MAX_DELETE_BATCH_TABLES {
        return Err(InspectError::StructureLimit.into());
    }
    budget
        .charge_allocation(groups.len() as u64, size_of::<Table>() as u64)
        .map_err(InspectError::from)?;
    let mut tables = Vec::new();
    tables
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut removed = 0_usize;
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        removed = removed
            .checked_add(group.deletes.len())
            .filter(|count| *count <= NativeImage::MAX_DELETE_BATCH_ROWS)
            .ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if groups[..position]
            .iter()
            .any(|prior| prior.table.eq_ignore_ascii_case(group.table))
        {
            return Err(EditError::Conflict);
        }
        let before = match &mut proven {
            Some(checks) => checks.next().ok_or(InspectError::Malformed)?,
            None => {
                let before = gates::before_delete(&image, group.table, budget, cancelled)?;
                relationships::qualify(&image, before.schema().name(), budget, cancelled)?;
                before
            }
        };
        if !before.schema().name().eq_ignore_ascii_case(group.table) {
            return Err(InspectError::Malformed.into());
        }
        rows::unique(group.deletes, budget, cancelled)?;
        for request in group.deletes {
            check(budget, cancelled)?;
            if request.expected_bytes().len() > PAGE_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
            budget
                .charge(Resource::Steps, request.expected_bytes().len() as u64)
                .map_err(InspectError::from)?;
        }
        tables.push(Table {
            before,
            references: Vec::new(),
            indexes: DeleteIndexPlan::default(),
        });
    }

    if proven
        .as_mut()
        .is_some_and(|checks| checks.next().is_some())
    {
        return Err(InspectError::Malformed.into());
    }
    let mut records = Vec::new();
    // Capture every table's stale/raw identities before any index is planned.
    for (table, group) in tables.iter_mut().zip(groups) {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let (staged, references) =
            patches::records(&mut reader, table.before.schema(), group.deletes)?;
        budget
            .charge_allocation(staged.len() as u64, size_of::<patches::Patch>() as u64)
            .map_err(InspectError::from)?;
        records
            .try_reserve(staged.len())
            .map_err(|_| InspectError::Allocation)?;
        records.extend(staged);
        table.references = references;
    }
    let mut indexes: Vec<IndexDataPatch> = Vec::new();
    for table in &mut tables {
        table.indexes = index_delete_patches(
            &image,
            table.before.schema(),
            table.before.indexes(),
            &table.references,
            budget,
            cancelled,
        )?;
        budget
            .charge_allocation(
                table.indexes.indexes.len() as u64,
                size_of::<IndexDataPatch>() as u64,
            )
            .map_err(InspectError::from)?;
        indexes
            .try_reserve(table.indexes.indexes.len())
            .map_err(|_| InspectError::Allocation)?;
        indexes.append(&mut table.indexes.indexes);
    }
    budget
        .charge_allocation(tables.len() as u64, size_of::<RetirementTable<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut retirements = Vec::new();
    retirements
        .try_reserve_exact(tables.len())
        .map_err(|_| InspectError::Allocation)?;
    for table in &tables {
        retirements.push(RetirementTable {
            schema: table.before.schema(),
            indexes: table.before.indexes(),
            retired: &table.indexes.retired,
        });
    }
    compose_retirements(&image, &retirements, &mut indexes, budget, cancelled)?;
    budget
        .charge_allocation(tables.len() as u64, size_of::<&str>() as u64)
        .map_err(InspectError::from)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(tables.len())
        .map_err(|_| InspectError::Allocation)?;
    names.extend(tables.iter().map(|table| table.before.schema().name()));
    let scope = image.catalog().catalog().scope();
    let plan =
        image.finish_tables_updates(&names, &records, &indexes, removed, budget, cancelled)?;
    audit_global(
        plan.before_bytes(),
        plan.after_bytes(),
        &retirements,
        budget,
        cancelled,
    )?;
    for table in &tables {
        audit_retirements(
            plan.before_bytes(),
            plan.after_bytes(),
            scope,
            table.before.schema().name(),
            &table.indexes.retired,
            budget,
            cancelled,
        )?;
        audit_siblings(
            plan.before_bytes(),
            plan.after_bytes(),
            &table.indexes.siblings,
            &table.indexes.retired,
            budget,
            cancelled,
        )?;
    }
    Ok(Ace16DeletePreview { plan })
}
