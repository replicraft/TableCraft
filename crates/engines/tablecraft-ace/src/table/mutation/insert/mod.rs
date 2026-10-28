//! Explicit engineering INSERT with request-bound record and allocation proof.
mod audit;
mod compose;
mod counters;
mod placement;
mod policies;
mod primary_column;
mod record;
#[cfg(test)]
mod tests;

use super::{gates, preview};
use crate::{
    Ace16WritePreview, EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck,
    WriteBlocker, check_table, check_table_constraints,
    header::check,
    inspect_catalog,
    limits::MAX_LIVE_ROWS,
    table::{
        indexes::mutation::insertion::{self, InsertKey, PrimaryInsertion},
        mutation::variable_split::metadata::Metadata,
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// One complete new row in the captured native column order.
/// Values are neither defaulted nor coerced; the selected insertion profile
/// validates every carrier against the actual table before constructing bytes.
pub struct RowInsert {
    values: Vec<Value>,
}

impl RowInsert {
    /// Capture all explicit field values. Table-dependent admission happens later.
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    /// Explicit requested values, in native column order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

struct Staged {
    data: placement::Plan,
    indexes: PrimaryInsertion,
    counters: Vec<counters::Patch>,
}

impl NativeImage {
    /// Construct a separate, non-journalable ACE 16 INSERT engineering candidate.
    ///
    /// The current profile constructs complete explicit ordinary fixed-scalar
    /// and Short Text/Binary/Decimal records. Boolean uses its native value mask; other
    /// fixed fields retain exact carriers and nullable presence bits. The sole
    /// required unique ascending Int32 primary is selected from index metadata,
    /// including logical aliases, independently of column order. Leaf and branch
    /// trees admit recursive splits and one retained-root height increase per
    /// operation, within existing depth and resource limits. Defaults, generated
    /// fields, additional indexes and unsupported framing refuse. Decimal requires
    /// its exact declared scale and precision without conversion or rounding. DATA and index
    /// growth share one qualified EOF cursor and native map allocator.
    ///
    /// This does not enable normal saves or qualify required data macros or
    /// Microsoft Access behavior. Only explicit disposable offline copies should
    /// materialize the returned candidate.
    pub fn preview_ace16_insert_rows(
        self,
        table: &str,
        requests: &[RowInsert],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.preview.insert", rows = requests.len()).entered();
        let structural = preflight(&self, table, requests, budget, cancelled)?;
        let staged = prepare(&self, &structural, requests, budget, cancelled)?;

        finish(self, table, requests, &staged, budget, cancelled)
    }
}

fn preflight(
    image: &NativeImage,
    table: &str,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    if requests.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }

    let structural = gates::before_insert(image, table, budget, cancelled)?;
    record::Profile::bind(structural.schema(), budget, cancelled)?;

    let count = structural
        .rows()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    if count > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }

    Ok(structural)
}

fn prepare(
    image: &NativeImage,
    structural: &TableCheck,
    requests: &[RowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let id = primary_column::resolve(structural, budget, cancelled)?;
    let profile = record::Profile::bind(structural.schema(), budget, cancelled)?;
    policies::require_text_declarations(image, structural.schema(), requests, budget, cancelled)?;

    budget
        .charge_allocation(requests.len() as u64, size_of::<Vec<u8>>() as u64)
        .map_err(InspectError::from)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;

    for request in requests {
        check(budget, cancelled)?;
        records.push(profile.build(request.values(), budget, cancelled)?);
    }

    let metadata = Metadata::capture(
        image,
        structural.schema(),
        structural.indexes(),
        id,
        budget,
        cancelled,
    )?;

    let data = placement::prepare(
        image,
        structural.schema(),
        metadata,
        &records,
        budget,
        cancelled,
    )?;

    budget
        .charge_allocation(requests.len() as u64, size_of::<InsertKey<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(requests.len())
        .map_err(|_| InspectError::Allocation)?;

    for (request, row) in requests.iter().zip(&data.rows) {
        check(budget, cancelled)?;
        keys.push(InsertKey {
            reference: row.reference,
            value: &request.values()[id],
        });
    }

    let indexes = insertion::prepare(image, structural, id, &keys, budget, cancelled)?;

    let placement::Plan {
        pages,
        maps,
        append,
        rows,
    } = data;
    let (indexes, append) =
        indexes.continue_with_append(image, structural, id, append, budget, cancelled)?;
    let data = placement::Plan {
        pages,
        maps,
        append,
        rows,
    };

    let counters = counters::capture(
        image,
        structural,
        &indexes,
        requests.len(),
        budget,
        cancelled,
    )?;

    Ok(Staged {
        data,
        indexes,
        counters,
    })
}

fn finish(
    image: NativeImage,
    table: &str,
    requests: &[RowInsert],
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let after = compose::compose(&image, staged, budget, cancelled)?;
    audit::prove(&image, table, requests, staged, &after, budget, cancelled)?;

    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let checked = check_table(&mut Cursor::new(&after), scope, table, budget, cancelled)?;
    if checked
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

    staged
        .data
        .maps
        .audit_membership(&after, checked.schema(), budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        requests.len(),
    ))
}
