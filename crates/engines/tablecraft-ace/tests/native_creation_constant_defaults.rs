#![cfg(feature = "ace16-create")]

//! Full public creator acceptance against independently qualified source images.
use std::cell::Cell;
use tablecraft_ace::{
    CreateColumn, CreateDatabaseError, CreateTable, CreatedDatabase, InspectError,
    create_ace16_database, create_ace16_database_tables,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::creation_constant_defaults as fixtures;

const LIMITS: [u64; 3] = [1_000_000_000, 1_000_000, 1_000_000_000];

fn invoke(
    case: &str,
    multiple: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let defaults = fixtures::defaults(case);
    let columns = columns(case, &defaults);
    let row = fixtures::original_row();
    let rows = [row.as_slice()];
    if multiple {
        create_ace16_database_tables(
            &[CreateTable::new("Items", &columns, &rows)],
            budget,
            cancelled,
        )
    } else {
        create_ace16_database("Items", &columns, &rows, budget, cancelled)
    }
}

fn budget(limits: [u64; 3]) -> ResourceBudget {
    ResourceBudget::new(limits[0], limits[1], limits[2])
}

#[test]
#[ignore = "requires all 28 frozen constant-default source views"]
fn both_real_creators_match_all_four_constant_metadata_sources() {
    let sources = sources();
    for case in fixtures::CASES {
        for multiple in [false, true] {
            assert_eq!(
                invoke(case, multiple, &mut budget(LIMITS), &|| false)
                    .unwrap()
                    .as_bytes(),
                sources.bytes(case, "before")
            );
        }
    }
}

#[test]
#[ignore = "requires all 28 frozen constant-default source views"]
fn complete_public_constant_creators_have_exact_pools_and_cancellable_boundaries() {
    let sources = sources();
    for case in fixtures::CASES {
        for multiple in [false, true] {
            let expected = sources.bytes(case, "before");
            let mut measured = budget(LIMITS);
            let polls = Cell::new(0_usize);
            assert_eq!(
                invoke(case, multiple, &mut measured, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap()
                .as_bytes(),
                expected
            );
            let total = polls.get();
            assert!(total > 2);
            for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
                .into_iter()
                .enumerate()
            {
                let used = LIMITS[position] - measured.remaining(resource);
                assert!(used > 0);
                let mut exact = LIMITS;
                exact[position] = used;
                assert_eq!(
                    invoke(case, multiple, &mut budget(exact), &|| false)
                        .unwrap()
                        .as_bytes(),
                    expected
                );
                exact[position] -= 1;
                assert!(
                    matches!(invoke(case, multiple, &mut budget(exact), &|| false), Err(CreateDatabaseError::Inspect(InspectError::Limit(failure))) if failure.resource == resource)
                );
            }
            for stop in [1, total.div_ceil(2), total] {
                polls.set(0);
                assert!(matches!(
                    invoke(case, multiple, &mut budget(LIMITS), &|| {
                        polls.set(polls.get() + 1);
                        polls.get() == stop
                    }),
                    Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
                ));
            }
        }
    }
}

#[test]
fn constant_defaults_reject_wrong_carriers_and_bound_escaped_metadata_before_image_publication() {
    for (scalar, value) in [
        (ScalarType::Boolean, Value::Null),
        (ScalarType::Boolean, Value::Int32(0)),
        (ScalarType::Int64, Value::Int32(7)),
        (ScalarType::Currency, Value::Int64(7)),
    ] {
        assert!(matches!(
            CreateColumn::new("Value", scalar).with_default_literal(&value),
            Err(CreateDatabaseError::Carrier)
        ));
    }
    let value = Value::Int64(1);
    assert!(matches!(
        CreateColumn::short_text("Text", 16)
            .unwrap()
            .with_default_literal(&value),
        Err(CreateDatabaseError::Carrier)
    ));
    for text in ["x".repeat(254), "\"".repeat(127)] {
        let literal = Value::Text(text);
        let columns = [CreateColumn::short_text("Caption", 255)
            .unwrap()
            .with_default_literal(&literal)
            .unwrap()];
        let rows: [&[Value]; 0] = [];
        for multiple in [false, true] {
            let result = if multiple {
                create_ace16_database_tables(
                    &[CreateTable::new("Items", &columns, &rows)],
                    &mut budget(LIMITS),
                    &|| false,
                )
            } else {
                create_ace16_database("Items", &columns, &rows, &mut budget(LIMITS), &|| false)
            };
            assert!(matches!(result, Err(CreateDatabaseError::Declaration)));
        }
    }
}

fn columns<'a>(case: &str, defaults: &'a [Value]) -> Vec<CreateColumn<'a>> {
    let mut caption = CreateColumn::short_text("Caption", 16)
        .unwrap()
        .with_allow_zero_length(case != "required-empty")
        .unwrap()
        .with_default_literal(&defaults[3])
        .unwrap();
    if case == "required-empty" {
        caption = caption.with_required(true);
    }
    vec![
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::new("Flag", ScalarType::Boolean)
            .with_default_literal(&defaults[0])
            .unwrap(),
        CreateColumn::new("Large", ScalarType::Int64)
            .with_default_literal(&defaults[1])
            .unwrap(),
        CreateColumn::new("Money", ScalarType::Currency)
            .with_default_literal(&defaults[2])
            .unwrap(),
        caption,
    ]
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}
