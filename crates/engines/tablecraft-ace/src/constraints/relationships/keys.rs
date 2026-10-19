//! Private equality keys, not SQL coercions or native index sort encodings.
use crate::{InspectError, TableSnapshot, format::layout::INDEX_KEY_COLUMNS, header::check};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Guid, Value},
};
use tablecraft_schema::ScalarType;

// Sorting supplies a deterministic set/search order only. In particular this
// Guid byte order makes no claim about Access ORDER BY or its physical indexes.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Atom {
    Integer(i64),
    Guid([u8; Guid::BYTE_LEN]),
}
pub(super) type Key = [Option<Atom>; INDEX_KEY_COLUMNS];

pub(super) fn position(table: &TableSnapshot, name: &str) -> Option<usize> {
    table.columns().iter().position(|column| {
        column.name().eq_ignore_ascii_case(name)
            && matches!(
                column.data_type(),
                ScalarType::Byte
                    | ScalarType::Int16
                    | ScalarType::Int32
                    | ScalarType::Int64
                    | ScalarType::Guid
            )
    })
}

fn atom(value: &Value) -> Result<Option<Atom>, InspectError> {
    Ok(Some(match value {
        Value::Null => return Ok(None),
        Value::Byte(value) => Atom::Integer(i64::from(*value)),
        Value::Int16(value) => Atom::Integer(i64::from(*value)),
        Value::Int32(value) => Atom::Integer(i64::from(*value)),
        Value::Int64(value) => Atom::Integer(*value),
        Value::Guid(value) => Atom::Guid(value.to_bytes()),
        _ => return Err(InspectError::Malformed),
    }))
}

pub(super) fn keys(
    table: &TableSnapshot,
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Key>, InspectError> {
    budget.charge_allocation(table.rows().len() as u64, size_of::<Key>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(table.rows().len())
        .map_err(|_| InspectError::Allocation)?;
    for row in table.rows() {
        check(budget, cancelled)?;
        let mut key = [None; INDEX_KEY_COLUMNS];
        for (position, column) in columns.iter().enumerate() {
            check(budget, cancelled)?;
            key[position] = atom(row.get(*column).ok_or(InspectError::Malformed)?)?;
        }
        // All-NULL tuples remain exempt; partial NULLs remain exact components.
        if key.iter().any(Option::is_some) {
            output.push(key);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn key_domains_preserve_null_and_reject_unsupported_coercions() {
        assert!(
            atom(&Value::Guid(Guid::from_bytes([0; Guid::BYTE_LEN]))).unwrap()
                != atom(&Value::Int64(0)).unwrap()
        );
        assert!(atom(&Value::Null).unwrap().is_none());
        assert_eq!(
            atom(&Value::Text(
                "{00000000-0000-0000-0000-000000000000}".into()
            ))
            .err(),
            Some(InspectError::Malformed)
        );
    }
    proptest! {
        #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x475549444b455953),..Default::default()})]
        #[test]
        fn guid_equality_matches_all_canonical_bytes(a in any::<[u8;Guid::BYTE_LEN]>(),b in any::<[u8;Guid::BYTE_LEN]>()) {
            let left=atom(&Value::Guid(Guid::from_bytes(a))).unwrap();let right=atom(&Value::Guid(Guid::from_bytes(b))).unwrap();
            prop_assert!((left==right)==(a==b));
        }
    }
}
