#![cfg(feature = "ace16-create")]
//! Actual creators consume only prequalified independent DATE source expectations.
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_ace::{
    CreateColumn, CreateDatabaseError, CreateTable, CreatedDatabase, InspectError,
    create_ace16_database, create_ace16_database_tables, inspect_table_properties,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Double, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::creation_date_defaults as fixtures;

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const LIMITS: [u64; 3] = [1_000_000_000, 1_000_000, 1_000_000_000];

fn budget(limits: [u64; 3]) -> ResourceBudget {
    ResourceBudget::new(limits[0], limits[1], limits[2])
}

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn invoke(
    bits: u64,
    multiple: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let literal = fixtures::date(bits);
    let columns = [
        CreateColumn::new("Id", ScalarType::Int32),
        CreateColumn::new("When", ScalarType::DateTime).with_default_literal(&literal)?,
    ];
    let row = fixtures::original_row();
    let rows = [row.as_slice()];
    if multiple {
        create_ace16_database_tables(
            &[CreateTable::new("Items", &columns, &rows)],
            work,
            cancelled,
        )
    } else {
        create_ace16_database("Items", &columns, &rows, work, cancelled)
    }
}

#[test]
#[ignore = "requires all source36 and root native bit/parser/Source qualification first"]
fn exact_date_declarations_match_six_literal_sources_in_both_real_creators() {
    let sources = sources();
    for case in fixtures::CASES {
        for multiple in [false, true] {
            let database = invoke(case.bits, multiple, &mut budget(LIMITS), &|| false).unwrap();
            assert_eq!(database.as_bytes(), sources.bytes(5, case, "before"));
            let properties = inspect_table_properties(
                &mut std::io::Cursor::new(database.as_bytes()),
                CatalogScope::new(230),
                "Items",
                &mut budget(LIMITS),
                &|| false,
            )
            .unwrap();
            assert_eq!(properties.groups().len(), 1);
            assert_eq!(properties.groups()[0].name(), "When");
            assert_eq!(
                properties.groups()[0]
                    .get("DefaultValue")
                    .unwrap()
                    .text()
                    .unwrap(),
                case.expression
            );
        }
    }
}

#[test]
#[ignore = "requires all source36 and root native bit/parser/Source qualification first"]
fn complete_date_creators_pay_exact_pools_and_cancel_before_artifact_return() {
    let sources = sources();
    for case in fixtures::CASES {
        let bits = case.bits;
        for multiple in [false, true] {
            let expected = sources.bytes(5, case, "before");
            let mut measured = budget(LIMITS);
            let polls = Cell::new(0_usize);
            assert_eq!(
                invoke(bits, multiple, &mut measured, &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap()
                .as_bytes(),
                expected
            );
            let costs: [u64; 3] =
                std::array::from_fn(|index| LIMITS[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));
            let mut exact = budget(costs);
            let created = invoke(bits, multiple, &mut exact, &|| false).unwrap();
            assert_eq!(created.as_bytes(), expected);
            assert_eq!(POOLS.map(|pool| exact.remaining(pool)), [0, 0, 0]);
            for index in 0..3 {
                let mut short = costs;
                short[index] -= 1;
                assert!(
                    matches!(invoke(bits, multiple, &mut budget(short), &|| false), Err(CreateDatabaseError::Inspect(InspectError::Limit(failure))) if failure.resource == POOLS[index])
                );
            }
            let terminal = polls.get();
            assert!(terminal > 2);
            for stop in [1, terminal.div_ceil(2), terminal] {
                polls.set(0);
                assert!(matches!(
                    invoke(bits, multiple, &mut budget(LIMITS), &|| {
                        polls.set(polls.get() + 1);
                        polls.get() >= stop
                    }),
                    Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
                ));
                assert_eq!(polls.get(), stop);
            }
        }
    }
}

#[test]
fn date_aliases_refuse_before_any_image_and_other_carriers_never_coerce() {
    for bits in fixtures::ALIAS_BITS {
        for multiple in [false, true] {
            assert!(matches!(
                invoke(bits, multiple, &mut budget(LIMITS), &|| false),
                Err(CreateDatabaseError::Declaration)
            ));
        }
    }
    for wrong in [
        Value::Double(Double::from_bits(0x3ff4000000000000)),
        Value::Int64(1),
        Value::Text("1899-12-31 06:00:00".into()),
    ] {
        assert!(matches!(
            CreateColumn::new("When", ScalarType::DateTime).with_default_literal(&wrong),
            Err(CreateDatabaseError::Carrier)
        ));
    }
    let null = Value::Null;
    let column = CreateColumn::new("When", ScalarType::DateTime)
        .with_default_literal(&null)
        .unwrap();
    assert!(std::ptr::eq(column.default_literal().unwrap(), &null));
}
