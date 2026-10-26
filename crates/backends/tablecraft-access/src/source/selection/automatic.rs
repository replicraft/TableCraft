//! Choose a matching declaration before traversal; never retry failed reads.
use super::{super::schema::ScalarSchema, candidate_selection};
use tablecraft_ace::{IndexInspection, IndexProjectionSelection, Int32IndexSelection};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::{QueryError, source::ScanConstraint};

pub(super) fn choose<'a>(
    inspection: &'a IndexInspection,
    declared: &ScalarSchema,
    constraint: ScanConstraint,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<(&'a str, IndexProjectionSelection<'static>)>, QueryError> {
    for index in inspection.indexes() {
        if cancelled() {
            return Err(QueryError::Cancelled);
        }
        // Candidate mapping searches native and exposed fields. Charge the
        // bounded metadata work even when a declaration is ultimately skipped.
        budget.charge(
            Resource::Steps,
            (inspection.schema().columns().len() + declared.columns.len() + 1) as u64,
        )?;
        let Some(constraints) = index.constraints() else {
            continue;
        };
        if index.columns().len() != 1 {
            continue;
        }

        let selection = candidate_selection(inspection, index, declared, constraint);
        if constraints.ignore_nulls()
            && !matches!(
                selection,
                IndexProjectionSelection::Int32(Int32IndexSelection::Range { .. })
            )
        {
            continue;
        }
        if selection != IndexProjectionSelection::All {
            return Ok(Some((index.name(), selection)));
        }
    }

    Ok(None)
}
