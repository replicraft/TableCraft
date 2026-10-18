use super::*;
use proptest::prelude::*;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_048_576, 1000, 100_000)
}
fn columns() -> Vec<Column> {
    COLUMNS
        .iter()
        .map(|name| {
            Column::new(
                (*name).into(),
                if matches!(*name, "grbit" | "ccolumn" | "icolumn") {
                    ScalarType::Int32
                } else {
                    ScalarType::Text
                },
                false,
            )
            .unwrap()
        })
        .collect()
}
fn row(count: i32, ordinal: i32) -> Vec<Value> {
    vec![
        Value::Text("ParentChild".into()),
        Value::Int32(0),
        Value::Int32(count),
        Value::Int32(ordinal),
        Value::Text("Parent".into()),
        Value::Text("Child".into()),
        Value::Text(format!("P{ordinal}")),
        Value::Text(format!("C{ordinal}")),
    ]
}

#[test]
fn unordered_composite_rows_retain_referenced_to_dependent_direction() {
    let relationships = decode(&columns(), &[row(2, 1), row(2, 0)], &mut budget(), &|| {
        false
    })
    .unwrap();
    let relationship = relationships[0].declaration();
    assert_eq!(relationship.parent_table().as_str(), "Parent");
    assert_eq!(relationship.child_table().as_str(), "Child");
    assert_eq!(relationship.columns()[0].parent().as_str(), "P0");
    assert_eq!(relationship.columns()[1].child().as_str(), "C1");
    assert!(relationship.rules().unwrap().enforced);
}

#[test]
fn malformed_ordinal_counts_metadata_and_keys_fail_without_partial_declarations() {
    for rows in [
        vec![row(2, 0)],
        vec![row(2, 0), row(2, 0)],
        vec![row(0, 0)],
        vec![row(257, 0)],
        vec![row(-1, 0)],
        vec![row(1, -1)],
        vec![row(1, 1)],
    ] {
        assert!(decode(&columns(), &rows, &mut budget(), &|| false).is_err());
    }
    for position in [1, 2, 4, 5] {
        let mut changed = row(2, 1);
        changed[position] = match position {
            1 => Value::Int32(2),
            2 => Value::Int32(3),
            _ => Value::Text("Other".into()),
        };
        assert!(decode(&columns(), &[row(2, 0), changed], &mut budget(), &|| false).is_err());
    }
    for position in 0..8 {
        let mut changed = row(1, 0);
        changed[position] = Value::Null;
        assert!(decode(&columns(), &[changed], &mut budget(), &|| false).is_err());
    }
    let mut duplicate = row(2, 1);
    duplicate[6] = Value::Text("P0".into());
    assert!(matches!(
        decode(&columns(), &[row(2, 0), duplicate], &mut budget(), &|| {
            false
        }),
        Err(InspectError::Schema(_))
    ));
}

#[test]
fn unknown_flags_cancellation_and_budgets_are_not_silently_disabled() {
    let mut unknown = row(1, 0);
    unknown[1] = Value::Int32(i32::MIN);
    let parsed = decode(&columns(), &[unknown], &mut budget(), &|| false).unwrap();
    assert_eq!(parsed[0].flags(), 0x8000_0000);
    assert!(parsed[0].declaration().rules().is_none());
    assert!(matches!(
        decode(&columns(), &[row(1, 0)], &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        decode(
            &columns(),
            &[row(1, 0)],
            &mut ResourceBudget::new(0, 0, 1000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        decode(
            &columns(),
            &[row(1, 0)],
            &mut ResourceBudget::new(1000, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x52454c4154),..Default::default()})]
    #[test]
    fn complete_composite_mappings_follow_ordinals_independent_of_row_order(count in 1i32..17, shift in 0usize..16, reverse in any::<bool>()) {
        let mut rows=(0..count).map(|ordinal|row(count,ordinal)).collect::<Vec<_>>();
        rows.rotate_left(shift%(count as usize));
        if reverse { rows.reverse(); }
        let parsed=decode(&columns(),&rows,&mut budget(),&||false).unwrap();
        prop_assert_eq!(parsed.len(),1);
        let pairs=parsed[0].declaration().columns();
        prop_assert_eq!(pairs.len(),count as usize);
        for (ordinal,pair) in pairs.iter().enumerate() {
            prop_assert_eq!(pair.parent().as_str(),format!("P{ordinal}"));
            prop_assert_eq!(pair.child().as_str(),format!("C{ordinal}"));
        }
    }
}
