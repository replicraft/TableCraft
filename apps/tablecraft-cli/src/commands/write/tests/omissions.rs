//! Presence is protocol intent, never a default value or generated marker.
use super::{Error, budget, input};
use std::cell::Cell;
use tablecraft_ace::{InsertFieldValue, InspectError, RowInsert};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[test]
fn omitted_marker_supplied_null_and_literal_text_remain_distinct_across_rows() {
    let encoded = b"int32\t4\tomitted\t\tnull\t\nint32\t5\ttext\tomitted\tnull\t\n";
    let parsed =
        input::presence::inserts(encoded, 1024, RowInsert::new, &mut budget(), &|| false).unwrap();
    let input::presence::Parsed::Omitted(rows) = parsed else {
        panic!("Original omission lost");
    };
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[0].values()[1], InsertFieldValue::Omitted));
    assert!(matches!(
        rows[0].values()[2],
        InsertFieldValue::Supplied(Value::Null)
    ));
    match &rows[1].values()[1] {
        InsertFieldValue::Supplied(Value::Text(text)) => assert_eq!(text, "omitted"),
        _ => panic!("Text DATA must remain literal"),
    }
    assert!(matches!(
        rows[1].values()[2],
        InsertFieldValue::Supplied(Value::Null)
    ));
}

#[test]
fn complete_rows_remain_complete_and_bad_presence_framing_refuses() {
    let input::presence::Parsed::Complete(rows) = input::presence::inserts(
        b"text\tomitted\tnull\t\n",
        1024,
        RowInsert::new,
        &mut budget(),
        &|| false,
    )
    .unwrap() else {
        panic!("Supplied fields became omissions");
    };
    assert_eq!(
        rows[0].values(),
        &[Value::Text("omitted".into()), Value::Null]
    );
    for invalid in [
        b"omitted\tnot-empty\n".as_slice(),
        b"omitted\tnull\n",
        b"Omitted\t\n",
        b"null\tnull\n",
        b"omitted\t\nint32\t4\tomitted\t\n",
        b"omitted\t\"unfinished",
    ] {
        assert!(matches!(
            input::presence::inserts(invalid, 1024, RowInsert::new, &mut budget(), &|| false),
            Err(Error::Input(_))
        ));
    }
}

#[test]
fn original_presence_and_complete_conversion_pay_all_pools_and_final_poll() {
    for encoded in [
        b"int32\t4\tomitted\t\nint32\t5\tnull\t\n".as_slice(),
        b"int32\t4\ttext\tomitted\nint32\t5\tnull\t\n".as_slice(),
    ] {
        let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
        let limits = [64 * 1024 * 1024, 100_000, 100_000_000];
        let mut work = ResourceBudget::new(limits[0], limits[1], limits[2]);
        let polls = Cell::new(0);
        input::presence::inserts(encoded, 1024, RowInsert::new, &mut work, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        for index in 0..3 {
            let needed = limits[index] - work.remaining(resources[index]);
            assert!(needed > 0);
            let mut exact = limits;
            exact[index] = needed;
            input::presence::inserts(
                encoded,
                1024,
                RowInsert::new,
                &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                &|| false,
            )
            .unwrap();
            exact[index] -= 1;
            assert!(matches!(
                input::presence::inserts(
                    encoded,
                    1024,
                    RowInsert::new,
                    &mut ResourceBudget::new(exact[0], exact[1], exact[2]),
                    &|| false
                ),
                Err(Error::Native(InspectError::Limit(_)))
            ));
        }
        let stop = polls.get();
        let calls = Cell::new(0);
        assert!(matches!(
            input::presence::inserts(encoded, 1024, RowInsert::new, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            }),
            Err(Error::Native(InspectError::Cancelled))
        ));
    }
}
