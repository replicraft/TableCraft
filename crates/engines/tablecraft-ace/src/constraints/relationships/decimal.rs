//! Wider equality atoms selected only for tuples containing Decimal fields.
use super::{keys, membership::validate_keys};
use crate::{InspectError, NativeColumn, ReadProfile, TableSnapshot};
use tablecraft_core::{limits::ResourceBudget, values::Value};

// Paired native fields must have identical precision/scale before this path is
// selected. Retain scale anyway: this private set order does not implement SQL
// coercion, decimal arithmetic or the physical index encoding.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum Atom {
    Compact(keys::Atom),
    Decimal { coefficient: i128, scale: u8 },
}

pub(super) fn compatible(parent: &NativeColumn, child: &NativeColumn) -> bool {
    let parameters = parent.decimal_parameters();
    parameters == child.decimal_parameters()
        && (parameters.is_none()
            || [parent, child].iter().all(|column| {
                matches!(
                    column.read_profile(),
                    ReadProfile::Jet4
                        | ReadProfile::Ace2007
                        | ReadProfile::Ace2016
                        | ReadProfile::Ace2019
                )
            }))
}

fn atom(value: &Value) -> Result<Option<Atom>, InspectError> {
    match value {
        Value::Decimal(value) => Ok(Some(Atom::Decimal {
            coefficient: value.coefficient(),
            scale: value.scale(),
        })),
        value => keys::atom(value).map(|value| value.map(Atom::Compact)),
    }
}

pub(super) fn check_keys(
    parent: &TableSnapshot,
    parent_columns: &[usize],
    child: &TableSnapshot,
    child_columns: &[usize],
    one_to_one: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let parents = keys::collect(parent, parent_columns, budget, cancelled, atom)?;
    let children = keys::collect(child, child_columns, budget, cancelled, atom)?;
    validate_keys(parents, children, one_to_one, budget, cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal};

    #[test]
    fn decimal_atoms_retain_scale_and_domain_even_for_zero() {
        let zero = |scale| atom(&Value::Decimal(Decimal::new(0, scale).unwrap())).unwrap();
        assert!(zero(0) != zero(28));
        assert!(zero(0) != atom(&Value::Int64(0)).unwrap());
        assert!(zero(4) != atom(&Value::Currency(Currency::from_scaled(0))).unwrap());
        assert!(zero(4) != atom(&Value::Null).unwrap());
        assert!(atom(&Value::Text("0".into())).is_err());
    }
}
