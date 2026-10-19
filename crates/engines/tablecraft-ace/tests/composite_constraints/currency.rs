use super::*;
use proptest::prelude::*;
use std::collections::BTreeSet;

type MoneyPair = (Option<i64>, Option<i64>);

fn currency_database(crossed: bool, values: &[MoneyPair]) -> Vec<u8> {
    let mut bytes = database(crossed);
    put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    for position in 0..2 {
        let field = 10 * 4096 + 75 + position * 25;
        bytes[field] = 5;
        put16(&mut bytes, field + 21, position as u16 * 8);
        put16(&mut bytes, field + 23, 8);
    }
    let rows = values
        .iter()
        .map(|(a, b)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(a.unwrap_or(0).to_le_bytes());
            row.extend(b.unwrap_or(0).to_le_bytes());
            row.push(u8::from(a.is_some()) | u8::from(b.is_some()) << 1);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    bytes
}

#[test]
fn currency_tuples_preserve_scaled_extremes_one_quantum_null_shapes_and_exact_orphans() {
    let values = [
        (Some(i64::MIN), Some(i64::MAX)),
        (Some(i64::MAX), Some(i64::MIN)),
        (Some(9_007_199_254_740_992), Some(9_007_199_254_740_993)),
        (Some(9_007_199_254_740_993), Some(9_007_199_254_740_992)),
        (Some(0), None),
        (None, Some(0)),
        (None, None),
        (None, None),
    ];
    for code in [5, 6] {
        for crossed in [false, true] {
            let mut bytes = currency_database(crossed, &values);
            bytes[20] = code;
            assert_eq!(
                audit(bytes).unwrap().relationships()[0].status(),
                RelationshipCheckStatus::Checked
            );
        }
    }
    assert_eq!(
        audit(currency_database(true, &[(Some(0), Some(1))])).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        audit(currency_database(true, &[(Some(0), None)])).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
}

#[test]
fn currency_partial_and_complete_duplicate_parent_tuples_are_rejected() {
    for pair in [
        (Some(i64::MAX), Some(i64::MIN)),
        (None, Some(0)),
        (Some(1), None),
    ] {
        assert_eq!(
            audit(currency_database(false, &[pair, pair])).err(),
            Some(InspectError::Constraint(
                ConstraintViolation::DuplicateParent
            ))
        );
    }
}

#[test]
fn mixed_currency_int64_tuples_require_matching_semantic_types_despite_equal_widths() {
    let values = [
        (Some(i64::MIN), Some(i64::MAX)),
        (Some(i64::MAX), Some(i64::MIN)),
    ];
    for crossed in [false, true] {
        let mut bytes = currency_database(crossed, &values);
        bytes[10 * 4096 + 75 + 25] = 0x13;
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            if crossed {
                RelationshipCheckStatus::Unsupported
            } else {
                RelationshipCheckStatus::Checked
            }
        );
    }
}

#[test]
fn mixed_currency_guid_tuples_retain_distinct_exact_carriers() {
    for crossed in [false, true] {
        let mut bytes = currency_database(crossed, &[(Some(-1), Some(0)), (Some(1), Some(0))]);
        let field = 10 * 4096 + 75 + 25;
        bytes[field] = 15;
        put16(&mut bytes, field + 23, 16);
        let rows = [(-1_i64, [0; 16]), (1_i64, [0xff; 16])]
            .into_iter()
            .map(|(money, guid)| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(money.to_le_bytes());
                row.extend(uuid::Uuid::from_bytes(guid).to_bytes_le());
                row.push(3);
                (0, row)
            })
            .collect::<Vec<_>>();
        bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            if crossed {
                RelationshipCheckStatus::Unsupported
            } else {
                RelationshipCheckStatus::Checked
            }
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x435552525455504c),
        ..Default::default() })]
    #[test]
    fn native_currency_tuple_audits_match_independent_scaled_integer_sets(
        values in prop::collection::vec((prop::option::of(any::<i64>()),
            prop::option::of(any::<i64>())),0..12), crossed in any::<bool>(),
    ) {
        let parents = values.iter().copied().filter(|pair| *pair != (None,None)).collect::<Vec<_>>();
        let set = parents.iter().copied().collect::<BTreeSet<_>>();
        let expected = if set.len() != parents.len() {
            Some(InspectError::Constraint(ConstraintViolation::DuplicateParent))
        } else if parents.iter().any(|(a,b)| !set.contains(&if crossed {(*b,*a)} else {(*a,*b)})) {
            Some(InspectError::Constraint(ConstraintViolation::MissingParent))
        } else {None};
        let result = audit(currency_database(crossed,&values));
        if let Some(expected) = expected {prop_assert_eq!(result.err(),Some(expected));}
        else {prop_assert_eq!(result.unwrap().relationships()[0].status(),RelationshipCheckStatus::Checked);}
    }
}
