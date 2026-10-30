//! New protocol serialization never replaces caller expectations with snapshots.
use super::{Error, budget, updates};
use std::cell::Cell;
use tablecraft_ace::FittedColumnUpdates;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

fn cells(variable: &str) -> Vec<String> {
    [
        "Items",
        "Age",
        "3",
        "int32",
        "null",
        "40",
        "Items",
        "Name",
        "3",
        variable,
        "null",
        if variable == "binary" { "00ff" } else { "aa" },
    ]
    .map(str::to_owned)
    .to_vec()
}

#[test]
fn mixed_protocol_preserves_kind_null_expectations_and_original_ordinals() {
    for kind in ["text", "binary"] {
        let input = cells(kind);
        let owned = updates::mixed::prepare(&input, 1024, &mut budget(), &|| false).unwrap();
        assert_eq!(owned.table, "Items");
        let groups = owned.groups(&mut budget(), &|| false).unwrap();
        let [
            FittedColumnUpdates::Fixed { column, updates },
            FittedColumnUpdates::Variable {
                column: name,
                updates: variable,
            },
        ] = groups.as_slice()
        else {
            panic!("wrong typed groups")
        };
        assert_eq!(*column, "Age");
        assert_eq!(*name, "Name");
        assert_eq!(updates[0].row(), 3);
        assert_eq!(updates[0].expected(), &Value::Null);
        assert_eq!(updates[0].value(), &Value::Int32(40));
        assert_eq!(variable[0].row(), 3);
        assert_eq!(variable[0].expected(), &Value::Null);
        match variable[0].value() {
            Value::Text(value) => assert_eq!(value, "aa"),
            Value::Binary(value) => assert_eq!(value, &[0, 255]),
            _ => panic!("wrong variable carrier"),
        }
    }
}

#[test]
fn mixed_scope_conflicts_and_unknown_kinds_refuse_without_native_groups() {
    for change in [0, 1, 2, 3] {
        let mut input = cells("text");
        match change {
            0 => input[6] = "OtherTable".to_owned(),
            1 => {
                input[3] = "boolean".to_owned();
                input[5] = "not-a-boolean".to_owned();
            }
            2 => input[7] = "Age".to_owned(),
            3 => input[3] = "unknown-kind".to_owned(),
            _ => unreachable!(),
        }
        let result = updates::mixed::prepare(&input, 1024, &mut budget(), &|| false);
        assert!(matches!(result, Err(Error::Input(_))));
    }
    let mut input = cells("text");
    input[5] = "not-an-int32".to_owned();
    assert!(matches!(
        updates::mixed::prepare(&input, 1024, &mut budget(), &|| false),
        Err(Error::Input(_))
    ));
    assert!(updates::mixed::prepare(&[], 1024, &mut budget(), &|| false).is_err());
}

#[test]
fn mixed_parser_and_borrowed_groups_obey_exact_pools_and_final_cancel() {
    let input = cells("binary");
    let limits = [64 * 1024 * 1024, 100_000, 100_000_000];
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let mut work = budget();
    let polls = Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        false
    };
    let owned = updates::mixed::prepare(&input, 1024, &mut work, &cancelled).unwrap();
    owned.groups(&mut work, &cancelled).unwrap();
    let needed =
        std::array::from_fn::<_, 3, _>(|index| limits[index] - work.remaining(resources[index]));

    for index in 0..3 {
        let mut exact = limits;
        exact[index] = needed[index];
        let mut work = ResourceBudget::new(exact[0], exact[1], exact[2]);
        let owned = updates::mixed::prepare(&input, 1024, &mut work, &|| false).unwrap();
        owned.groups(&mut work, &|| false).unwrap();
        exact[index] -= 1;
        let mut work = ResourceBudget::new(exact[0], exact[1], exact[2]);
        let result = updates::mixed::prepare(&input, 1024, &mut work, &|| false)
            .and_then(|owned| owned.groups(&mut work, &|| false).map(|_| ()));
        assert!(result.is_err());
    }

    let last = polls.get();
    let polls = Cell::new(0);
    let mut work = budget();
    let cancelled = || {
        polls.set(polls.get() + 1);
        polls.get() == last
    };
    let result = updates::mixed::prepare(&input, 1024, &mut work, &cancelled)
        .and_then(|owned| owned.groups(&mut work, &cancelled).map(|_| ()));
    assert!(matches!(
        result,
        Err(Error::Native(tablecraft_ace::InspectError::Cancelled))
    ));
}

#[test]
fn variable_only_multiple_columns_select_one_joint_protocol() {
    let mut input = cells("text");
    input[1] = "First".to_owned();
    input[3] = "text".to_owned();
    input[5] = "a".to_owned();
    assert!(updates::mixed::required(&input, &mut budget(), &|| false).unwrap());
    let owned = updates::mixed::prepare(&input, 1024, &mut budget(), &|| false).unwrap();
    let groups = owned.groups(&mut budget(), &|| false).unwrap();
    assert_eq!(groups.len(), 2);
    assert!(
        groups
            .iter()
            .all(|group| matches!(group, FittedColumnUpdates::Variable { .. }))
    );
}
