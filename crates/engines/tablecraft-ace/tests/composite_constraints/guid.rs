use super::*;
use proptest::prelude::*;
use std::collections::BTreeSet;

type GuidPair = (Option<[u8; 16]>, Option<[u8; 16]>);

fn guid_database(crossed: bool, values: &[GuidPair]) -> Vec<u8> {
    let mut bytes = database(crossed);
    put32(&mut bytes, 10 * 4096 + 16, values.len() as u32);
    for (position, offset) in [(0, 0), (1, 16)] {
        let field = 10 * 4096 + 75 + position * 25;
        bytes[field] = 15;
        put16(&mut bytes, field + 21, offset);
        put16(&mut bytes, field + 23, 16);
    }
    let rows = values
        .iter()
        .map(|(a, b)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            for value in [a, b] {
                row.extend(uuid::Uuid::from_bytes(value.unwrap_or([0; 16])).to_bytes_le());
            }
            row.push(u8::from(a.is_some()) | u8::from(b.is_some()) << 1);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&data_page(10, &rows));
    bytes
}

#[test]
fn guid_tuples_preserve_all_bits_null_shapes_uniqueness_and_exact_orphans() {
    let a = Some([0; 16]);
    let b = Some([0xff; 16]);
    let c = Some([0x80; 16]);
    let values = [
        (a, b),
        (b, a),
        (c, None),
        (None, c),
        (None, None),
        (None, None),
    ];
    for code in [5, 6] {
        for crossed in [false, true] {
            let mut bytes = guid_database(crossed, &values);
            bytes[20] = code;
            assert_eq!(
                audit(bytes).unwrap().relationships()[0].status(),
                RelationshipCheckStatus::Checked
            );
        }
    }
    assert_eq!(
        audit(guid_database(true, &[(a, b)])).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        audit(guid_database(false, &[(a, b), (a, b)])).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn mixed_guid_integer_pairs_require_matching_storage_types() {
    for crossed in [false, true] {
        let mut bytes = guid_database(
            crossed,
            &[(Some([0x80; 16]), None), (Some([0xff; 16]), None)],
        );
        let field = 10 * 4096 + 75 + 25;
        bytes[field] = 4;
        put16(&mut bytes, field + 23, 4);
        let rows = [([0x80; 16], i32::MIN), ([0xff; 16], i32::MAX)]
            .into_iter()
            .map(|(guid, key)| {
                let mut row = 2_u16.to_le_bytes().to_vec();
                row.extend(uuid::Uuid::from_bytes(guid).to_bytes_le());
                row.extend(key.to_le_bytes());
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
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x475549445455504c),..Default::default()})]
    #[test]
    fn native_guid_tuple_audits_match_independent_byte_tuple_sets(values in prop::collection::vec((prop::option::of(any::<[u8;16]>()),prop::option::of(any::<[u8;16]>())),0..12),crossed in any::<bool>()) {
        let parents=values.iter().copied().filter(|pair|*pair!=(None,None)).collect::<Vec<_>>();let set=parents.iter().copied().collect::<BTreeSet<_>>();
        let expected=if set.len()!=parents.len(){Some(InspectError::Constraint(ConstraintViolation::DuplicateParent))}
            else if parents.iter().any(|(a,b)|!set.contains(&if crossed{(*b,*a)}else{(*a,*b)})){Some(InspectError::Constraint(ConstraintViolation::MissingParent))}else{None};
        let result=audit(guid_database(crossed,&values));
        if let Some(expected)=expected {prop_assert_eq!(result.err(),Some(expected));}else{prop_assert_eq!(result.unwrap().relationships()[0].status(),RelationshipCheckStatus::Checked);}
    }
}
