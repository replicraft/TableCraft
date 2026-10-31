//! Fresh program/intent replay supplies the sole physical byte authority.
use super::super::{copy, joint::proof as fixed_audit, source};
use crate::{
    BeforeChangePreviewError, BeforeEventGuardError, BeforeEventGuardSelector, FixedColumnUpdates,
    InspectError, NativeImage,
    header::check,
    inspect_table_schema,
    table::mutation::{
        guards::fields::batch::{self, Prepared, SelectedPrograms, SourceFacts},
        preview,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct Observed<'prepared, 'image> {
    pub prepared: &'prepared Prepared<'image>,
    pub bytes: &'prepared [u8],
    pub changed_rows: usize,
}

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    original_groups: &[FixedColumnUpdates<'_>],
    observed: Observed<'_, '_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeChangePreviewError> {
    check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
    preview::qualify(image)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )
    .map_err(BeforeEventGuardError::from)?;
    let selected = SelectedPrograms::capture(image, &schema, selectors, budget, cancelled)?;
    // The independent Source is captured with its own source-bound selection.
    // No producer Source, compiled object, frame or final-group cache enters it.
    let original = source::capture_before_change(image, table, &selected, budget, cancelled)?;
    observed.prepared.admission().require_source(
        image,
        original.table.schema(),
        budget,
        cancelled,
    )?;
    let replay = batch::prepare(
        image,
        SourceFacts {
            table: &original.table,
            indexes: &original.indexes,
            dependencies: &original.dependencies,
        },
        selected,
        original_groups,
        budget,
        cancelled,
    )?;
    audit(&replay, observed.prepared, budget, cancelled)?;
    // Even an accurately reported RaiseError cannot have a physical candidate.
    // The coordinator returns its typed rejection before entering this proof.
    if replay.rejection().is_some() {
        return Err(BeforeEventGuardError::Preservation.into());
    }
    let final_groups = replay.groups(original.table.schema(), budget, cancelled)?;
    let expected = copy(image.as_bytes(), budget, cancelled)?;
    fixed_audit::prove_original(
        fixed_audit::Original {
            image,
            source: &original,
        },
        expected,
        &final_groups,
        fixed_audit::Observed {
            bytes: observed.bytes,
            changed_rows: observed.changed_rows,
        },
        budget,
        cancelled,
    )?;
    // The reused auditor independently encodes only original expected carriers,
    // accumulates mask bits, counts actual changed roots and validates the final
    // expected projection against ORIGINAL properties. Its validation does not
    // call inert Source capture or remove/replace the selected Extra payload.
    check(budget, cancelled).map_err(BeforeEventGuardError::from)?;
    Ok(())
}

fn audit(
    expected: &Prepared<'_>,
    observed: &Prepared<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    let mut work = expected
        .programs()
        .len()
        .checked_add(observed.programs().len())
        .and_then(|count| count.checked_add(expected.trace().len()))
        .and_then(|count| count.checked_add(observed.trace().len()))
        .and_then(|count| count.checked_add(expected.assignments().len()))
        .and_then(|count| count.checked_add(observed.assignments().len()))
        .ok_or(InspectError::StructureLimit)?;
    for program in expected.programs().iter().chain(observed.programs()) {
        check(budget, cancelled)?;
        work = work
            .checked_add(program.name().len())
            .and_then(|count| count.checked_add(program.native_property_bytes().len()))
            .ok_or(InspectError::StructureLimit)?;
    }
    for rejection in [expected.rejection(), observed.rejection()]
        .into_iter()
        .flatten()
    {
        check(budget, cancelled)?;
        work = work
            .checked_add(rejection.description().map_or(0, str::len))
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    if expected.programs() != observed.programs()
        || expected.trace() != observed.trace()
        || expected.assignments() != observed.assignments()
        || expected.rejection() != observed.rejection()
    {
        return Err(BeforeEventGuardError::Preservation);
    }
    match (expected.rows(), observed.rows()) {
        (None, None) => {}
        (Some(expected), Some(observed)) if expected.len() == observed.len() => {
            for (expected, observed) in expected.iter().zip(observed) {
                check(budget, cancelled)?;
                if expected.ordinal() != observed.ordinal()
                    || expected.values().len() != observed.values().len()
                {
                    return Err(BeforeEventGuardError::Preservation);
                }
                for (expected, observed) in expected.values().iter().zip(observed.values()) {
                    equal_immediate(expected, observed, true, budget, cancelled)?;
                }
            }
        }
        _ => return Err(BeforeEventGuardError::Preservation),
    }
    match (expected.final_groups(), observed.final_groups()) {
        (None, None) => {}
        (Some(expected), Some(observed)) if expected.len() == observed.len() => {
            for (expected, observed) in expected.iter().zip(observed) {
                check(budget, cancelled)?;
                if expected.column() != observed.column()
                    || expected.updates().len() != observed.updates().len()
                {
                    return Err(BeforeEventGuardError::Preservation);
                }
                for (expected, observed) in expected.updates().iter().zip(observed.updates()) {
                    check(budget, cancelled)?;
                    if expected.row() != observed.row() {
                        return Err(BeforeEventGuardError::Preservation);
                    }
                    equal_immediate(
                        expected.expected(),
                        observed.expected(),
                        false,
                        budget,
                        cancelled,
                    )?;
                    equal_immediate(expected.value(), observed.value(), false, budget, cancelled)?;
                }
            }
        }
        _ => return Err(BeforeEventGuardError::Preservation),
    }
    check(budget, cancelled)?;
    Ok(())
}

fn equal_immediate(
    expected: &Value,
    observed: &Value,
    int32_only: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, size_of::<Value>() as u64)
        .map_err(InspectError::from)?;
    let admitted = |value: &Value| {
        matches!(value, Value::Null | Value::Int32(_))
            || (!int32_only
                && matches!(
                    value,
                    Value::Boolean(_) | Value::Int64(_) | Value::Currency(_) | Value::DateTime(_)
                ))
    };
    // Reject forged variable-size/opaque carriers before Value equality. No
    // uncharged Text/Binary comparison can hide in an observed report.
    if !admitted(expected) || !admitted(observed) || expected != observed {
        return Err(BeforeEventGuardError::Preservation);
    }
    Ok(())
}
