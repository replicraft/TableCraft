//! Shared current generated-row construction; source families remain explicit.
mod proof;

use super::{current, incrementing, indexed, retained};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    header::check,
    table::mutation::{
        autonumber_counter,
        preserved_source::Source,
        record_insertion::{self, properties},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy)]
pub(super) enum Family {
    Current,
    Retained,
}

impl Family {
    pub(super) fn capture(
        self,
        image: &NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source, EditError> {
        match self {
            Self::Current => current::capture(image, table, budget, cancelled),
            Self::Retained => retained::capture(image, table, budget, cancelled),
        }
    }
}

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    requests: &[OmittedRowInsert],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.generated.insert.plan").entered();
    let counter = incrementing::mode::capture_current(&image, &source, budget, cancelled)?;
    let variables = record_insertion::current::variables(
        source.table.schema(),
        counter.column,
        budget,
        cancelled,
    )?;
    incrementing::binding::validate(&image, &source, counter.column, requests, budget, cancelled)?;
    let generated = autonumber_counter::prepare(counter, requests.len(), budget, cancelled)?;
    let counter = generated.source;
    let rows =
        incrementing::binding::expand(&image, &source, &generated, requests, budget, cancelled)?;
    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
    let (facts, optional) = indexed::qualify(&image, &source, table, &rows, budget, cancelled)?;
    if optional {
        return Err(EditError::Preservation);
    }

    let mut records = incrementing::vector(rows.len(), budget, cancelled)?;
    for row in &rows {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        records.push(record_insertion::current::build(
            source.table.schema(),
            &variables,
            row.values(),
            budget,
            cancelled,
        )?);
    }
    let mut after = indexed::stage(&image, &source, &rows, &records, &facts, budget, cancelled)?;
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, (2 * size_of::<i32>()) as u64)
        .map_err(InspectError::from)?;
    let stored = after
        .get_mut(counter.offset..counter.offset + size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    if stored != counter.before {
        return Err(EditError::Preservation);
    }
    stored.copy_from_slice(&generated.after);
    drop(facts);

    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: requests.len(),
    };
    proof::prove(&plan, scope, table, requests, family, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}
