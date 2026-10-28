//! Bind complete fixed-key profiles to one canonical physical owner.
use super::*;
use crate::table::indexes::{
    composite::{DecimalColumns, DecimalValue, MixedValue},
    decimal::{DecimalIndexSelection, DecimalProfile, DecimalType},
    guid::{GuidIndexSelection, GuidProfile},
    mutation::composite::TupleProfile,
    profile::Coverage,
};

pub(super) enum Profile {
    Numeric(SingleProfile),
    Decimal(DecimalProfile),
    Guid(GuidProfile),
    Tuple(TupleProfile),
    Mixed(TupleProfile<MixedValue>),
    DecimalTuple(TupleProfile<DecimalValue, DecimalColumns>),
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
        if column.scalar_type() == Some(ScalarType::Decimal) {
            return Ok(Profile::Decimal(DecimalProfile {
                column: position,
                descending: key.descending(),
                kind: DecimalType::from_column(column)?,
                selection: DecimalIndexSelection::All,
            }));
        }
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
    let mut decimal = false;
    for component in index.columns() {
        let mut found = false;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.number() == component.number() {
                guid |= field.scalar_type() == Some(ScalarType::Guid);
                decimal |= field.scalar_type() == Some(ScalarType::Decimal);
                found = true;
            }
        }
        if !found {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    if decimal {
        let profile = TupleProfile::<DecimalValue, DecimalColumns>::with_decimal_coverage(
            index, schema, coverage, budget, cancelled,
        )?;
        Ok(Profile::DecimalTuple(profile))
    } else if guid {
        Ok(Profile::Mixed(TupleProfile::<MixedValue>::with_coverage(
            index, schema, coverage, budget, cancelled,
        )?))
    } else {
        Ok(Profile::Tuple(TupleProfile::with_coverage(
            index, schema, coverage, budget, cancelled,
        )?))
    }
}
