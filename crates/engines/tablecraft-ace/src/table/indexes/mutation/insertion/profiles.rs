//! Bind complete fixed-key profiles to one canonical physical owner.
use super::*;
use crate::format::data_type::{LONG_TEXT, MAX_SHORT_BINARY_BYTES, SHORT_BINARY, SHORT_TEXT};
use crate::table::indexes::{
    binary::{BinaryIndexSelection, BinaryProfile},
    composite::{DecimalColumns, DecimalValue, MixedValue},
    decimal::{DecimalIndexSelection, DecimalProfile, DecimalType},
    encoded_tuple::EncodedTupleProfile,
    guid::{GuidIndexSelection, GuidProfile},
    mutation::composite::TupleProfile,
    mutation::retained::Preserved,
    profile::Coverage,
    text::TextProfile,
};
use tablecraft_core::limits::Resource;

pub(super) enum Profile<'a> {
    Numeric(SingleProfile),
    Text(Preserved<TextProfile>),
    Binary(Preserved<BinaryProfile<'static>>),
    Encoded(Preserved<EncodedTupleProfile<'a>>),
    Decimal(DecimalProfile),
    Guid(GuidProfile),
    Tuple(TupleProfile),
    Mixed(TupleProfile<MixedValue>),
    DecimalTuple(TupleProfile<DecimalValue, DecimalColumns>),
}

pub(super) fn bind<'a>(
    checked: &'a TableCheck,
    index: &NativeIndex,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Profile<'a>, EditError> {
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
        if column.native_type() == SHORT_TEXT && column.scalar_type() == Some(ScalarType::Text) {
            return Ok(Profile::Text(Preserved(TextProfile::new(
                position,
                column,
                key.descending(),
            )?)));
        }
        if column.native_type() == SHORT_BINARY && column.scalar_type() == Some(ScalarType::Binary)
        {
            let maximum = column.storage_length();
            if maximum == 0 || maximum > MAX_SHORT_BINARY_BYTES {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            return Ok(Profile::Binary(Preserved(BinaryProfile {
                column: position,
                descending: key.descending(),
                maximum,
                selection: BinaryIndexSelection::All,
            })));
        }
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
    let mut variable = false;
    let mut guid = false;
    let mut decimal = false;
    for component in index.columns() {
        let mut found = false;
        for field in schema.columns() {
            check(budget, cancelled)?;
            if field.number() == component.number() {
                if field.native_type() == LONG_TEXT {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                variable |= matches!(field.native_type(), SHORT_TEXT | SHORT_BINARY);
                guid |= field.scalar_type() == Some(ScalarType::Guid);
                decimal |= field.scalar_type() == Some(ScalarType::Decimal);
                found = true;
            }
        }
        if !found {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    if variable {
        budget
            .charge(
                Resource::Steps,
                (index.columns().len() * schema.columns().len()) as u64,
            )
            .map_err(InspectError::from)?;
        Ok(Profile::Encoded(Preserved(EncodedTupleProfile::new(
            index, schema, coverage,
        )?)))
    } else if decimal {
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
