//! Convert a numeric Decimal equality hint to an exact native field point.
use tablecraft_ace::{DecimalIndexSelection, NativeColumn};
use tablecraft_core::values::Decimal;
use tablecraft_schema::{DecimalConstraints, ScalarType};

pub(super) fn equality(field: &NativeColumn, value: Decimal) -> Option<DecimalIndexSelection> {
    if field.scalar_type() != Some(ScalarType::Decimal) {
        return None;
    }
    let (precision, scale) = field.decimal_parameters()?;
    let declared = DecimalConstraints::new(precision, scale).ok()?;
    let point = value.rescale_exact(scale).ok()?;
    declared.validate(point).ok()?;
    Some(DecimalIndexSelection::Equal(point))
}
