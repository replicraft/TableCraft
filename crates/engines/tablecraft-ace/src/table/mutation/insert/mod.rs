//! Explicit engineering INSERT with request-bound record and allocation proof.
mod audit;
mod autonumber;
mod compose;
mod counters;
mod incrementing;
mod indexes;
mod long_values;
mod placement;
mod policies;
pub(super) mod primary_column;
mod record;
#[cfg(test)]
pub(in crate::table::mutation) mod tests;

use super::{gates, preview};
use crate::{
    Ace16WritePreview, EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck,
    WriteBlocker, check_table, check_table_constraints,
    header::check,
    inspect_catalog,
    limits::MAX_LIVE_ROWS,
    table::{
        indexes::mutation::insertion::IndexInsertion,
        mutation::{append::State, variable_split::metadata::Metadata},
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub use autonumber::{
    Ace16AutoNumberInsertPreview, Ace16IncrementingInsertBinding, AutoNumberRowInsert,
    IncrementingAutoNumber,
};

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
    long_values: long_values::Plan,
    indexes: Vec<IndexInsertion>,
    counters: Vec<counters::Patch>,
    autonumber: Option<autonumber::counter::Plan>,
}

#[derive(Clone, Copy)]
enum AuditRequest<'a> {
    Explicit(&'a [RowInsert]),
    Incrementing {
        requests: &'a [AutoNumberRowInsert],
        policy: IncrementingAutoNumber,
    },
}

impl NativeImage {
    /// Construct a separate, non-journalable ACE 16 INSERT engineering candidate.
    ///
    /// Construct complete explicit qualified fixed scalars, Decimal, Short
    /// Text/Binary and ordinary Memo/OLE values without coercion or defaults.
    /// Memo/OLE descriptors remain inline or use checked column-owned external
    /// chunks. External long values, ordinary DATA and each physical INDEX owner
    /// share one cursor in that order; requests without external long values
    /// retain DATA-before-INDEX allocation.
    ///
    /// One required unique ascending Int32 primary is resolved from actual
    /// metadata. Qualified fixed and variable secondary keys retain their own
    /// directions, NULL rules and conservative digest-uniqueness restrictions.
    /// Non-required Ignore-NULL secondary indexes omit only all-NULL keys;
    /// Boolean values remain non-NULL. Logical aliases share each physical plan,
    /// recursive splits and historical distinct-key counter changes.
    ///
    /// Unknown metadata, unsupported storage/maps, defaults, generated behavior
    /// and required active validation remain explicit refusal boundaries.
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
    let primary = primary_column::resolve(structural, budget, cancelled)?;
    let profile = record::Profile::bind(structural.schema(), budget, cancelled)?;
    prepare_with(
        image, structural, requests, primary, profile, budget, cancelled,
    )
}

fn prepare_with(
    image: &NativeImage,
    structural: &TableCheck,
    requests: &[RowInsert],
    primary: usize,
    profile: record::Profile<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
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
        if !matches!(request.values().get(primary), Some(Value::Int32(_))) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    let has_long = structural
        .schema()
        .columns()
        .iter()
        .any(crate::table::long_value::profile::ordinary);
    let (long_values, append) = if has_long {
        budget
            .charge_allocation(
                requests.len() as u64,
                size_of::<record::input::Input>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(requests.len())
            .map_err(|_| InspectError::Allocation)?;
        for request in requests {
            inputs.push(profile.prepare_input(request.values(), budget, cancelled)?);
        }
        let (long_values, append) = long_values::prepare(
            image,
            structural,
            profile.variables(),
            &mut inputs,
            budget,
            cancelled,
        )?;
        for (request, input) in requests.iter().zip(inputs) {
            records.push(profile.build_prepared(request.values(), input, budget, cancelled)?);
        }
        (long_values, append)
    } else {
        for request in requests {
            records.push(profile.build(request.values(), budget, cancelled)?);
        }
        (
            long_values::Plan {
                qualified_zero_columns: Vec::new(),
                qualified_missing_columns: Vec::new(),
                definition_growth: None,
                initializations: Vec::new(),
                pages: Vec::new(),
                metadata: Vec::new(),
            },
            State::new(image, structural.schema().definition_page()),
        )
    };

    let metadata = Metadata::capture_for_insert_initialization(
        image,
        structural.schema(),
        structural.indexes(),
        &long_values.qualified_zero_columns,
        budget,
        cancelled,
    )?;

    let data = placement::prepare(
        image,
        structural.schema(),
        metadata,
        &records,
        append,
        budget,
        cancelled,
    )?;

    let placement::Plan {
        pages,
        maps,
        append,
        rows,
    } = data;
    let (indexes, append) = indexes::prepare(
        image, structural, requests, &rows, append, budget, cancelled,
    )?;
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
        long_values,
        indexes,
        counters,
        autonumber: None,
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
    finish_with(
        image,
        table,
        AuditRequest::Explicit(requests),
        staged,
        budget,
        cancelled,
    )
}

fn finish_with(
    image: NativeImage,
    table: &str,
    requests: AuditRequest<'_>,
    staged: &Staged,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let after = compose::compose(&image, staged, budget, cancelled)?;
    let inserted = match requests {
        AuditRequest::Explicit(requests) => {
            audit::prove(&image, table, requests, staged, &after, budget, cancelled)?;
            requests.len()
        }
        AuditRequest::Incrementing { requests, policy } => {
            audit::prove_incrementing(
                &image,
                table,
                (requests, policy),
                staged,
                &after,
                budget,
                cancelled,
            )?;
            requests.len()
        }
    };

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
        inserted,
    ))
}
