//! Projected fixed-key facts after complete before-image validation.
use super::{EditError, reserve};
use crate::{
    InspectError, NativeImage, NativeRelationship, TableSchema, TableSnapshot, header::check,
    read_table_projection,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ReferentialAction;

#[derive(Clone, Copy)]
pub(super) enum Purpose {
    Delete,
    Update,
}

pub(super) fn read(
    image: &NativeImage,
    schema: &TableSchema,
    relationships: &[NativeRelationship],
    purpose: Purpose,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableSnapshot, Vec<usize>), EditError> {
    let width = schema.columns().len();
    budget
        .charge_allocation(width as u64, size_of::<bool>() as u64)
        .map_err(InspectError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(width)
        .map_err(|_| InspectError::Allocation)?;
    selected.resize(width, false);

    for relationship in relationships {
        check(budget, cancelled)?;
        let declaration = relationship.declaration();
        // Admission remains in State::expand, with its existing error phases.
        // Unenforced/unknown declarations do not invent new lookup work here.
        if !declaration.rules().is_some_and(|rules| {
            rules.enforced
                && (matches!(purpose, Purpose::Update)
                    || matches!(
                        rules.on_delete,
                        ReferentialAction::Cascade | ReferentialAction::SetNull
                    ))
        }) {
            continue;
        }
        let parent = declaration
            .parent_table()
            .as_str()
            .eq_ignore_ascii_case(schema.name());
        let child = declaration
            .child_table()
            .as_str()
            .eq_ignore_ascii_case(schema.name());
        if !parent && !child {
            continue;
        }
        for pair in declaration.columns() {
            check(budget, cancelled)?;
            if parent {
                mark(
                    schema,
                    pair.parent().as_str(),
                    &mut selected,
                    budget,
                    cancelled,
                )?;
            }
            if child {
                mark(
                    schema,
                    pair.child().as_str(),
                    &mut selected,
                    budget,
                    cancelled,
                )?;
            }
        }
    }

    let mut positions = Vec::new();
    let mut names = Vec::new();
    for (position, included) in selected.into_iter().enumerate() {
        check(budget, cancelled)?;
        if included {
            reserve(&mut positions, budget)?;
            reserve(&mut names, budget)?;
            positions.push(position);
            names.push(schema.columns()[position].name());
        }
    }
    let snapshot = read_table_projection(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        &names,
        budget,
        cancelled,
    )?;
    if snapshot.columns().len() != positions.len() {
        return Err(InspectError::Malformed.into());
    }
    for (projected, &position) in snapshot.columns().iter().zip(&positions) {
        check(budget, cancelled)?;
        let native = &schema.columns()[position];
        if !projected.name().eq_ignore_ascii_case(native.name())
            || Some(projected.data_type()) != native.scalar_type()
        {
            return Err(InspectError::Malformed.into());
        }
    }
    Ok((snapshot, positions))
}

fn mark(
    schema: &TableSchema,
    name: &str,
    selected: &mut [bool],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for (position, column) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if column.name().eq_ignore_ascii_case(name) {
            selected[position] = true;
            return Ok(());
        }
    }
    Err(InspectError::Malformed.into())
}
