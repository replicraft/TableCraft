//! Bind complete fixed-key profiles to one canonical physical owner.
use super::*;
use crate::table::indexes::{
    composite::MixedValue,
    guid::{GuidIndexSelection, GuidProfile},
    mutation::composite::TupleProfile,
    profile::Coverage,
};

pub(super) enum Profile {
    Numeric(SingleProfile),
    Guid(GuidProfile),
    Tuple(TupleProfile),
    Mixed(TupleProfile<MixedValue>),
}

pub(super) fn bind(
    checked: &TableCheck,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Profile, EditError> {
    let schema = checked.schema();
    if let [key] = index.columns() {
        let mut selected = None;
        for (position, column) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if column.number() == key.number() {
                selected = Some((position, column));
            }
        }
        let (position, column) = selected.ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        if column.scalar_type() == Some(ScalarType::Guid) {
            return Ok(Profile::Guid(GuidProfile {
                column: position,
                descending: key.descending(),
                selection: GuidIndexSelection::All,
            }));
        }
        let kind = NumericKey::new(
            column
                .scalar_type()
                .ok_or(EditError::Unsupported(WriteBlocker::Column))?,
        )?;
        return Ok(Profile::Numeric(SingleProfile {
            column: position,
            kind,
            descending: key.descending(),
        }));
    }
    let coverage = if index
        .constraints()
        .ok_or(InspectError::Unsupported)?
        .ignore_nulls()
    {
        Coverage::PhysicalMembership
    } else {
        Coverage::CompleteTable
    };
    let mut guid = false;
    for component in index.columns() {
        let mut found = false;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.number() == component.number() {
                guid |= field.scalar_type() == Some(ScalarType::Guid);
                found = true;
            }
        }
        if !found {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    if guid {
        Ok(Profile::Mixed(TupleProfile::<MixedValue>::with_coverage(
            index, schema, coverage, budget, cancelled,
        )?))
    } else {
        Ok(Profile::Tuple(TupleProfile::with_coverage(
            index, schema, coverage, budget, cancelled,
        )?))
    }
}
