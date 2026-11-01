//! Full public creator measurement; no internal cost formula is an oracle.
use super::super::CreatedDatabase;
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables,
};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::canonical_creation_defaults as sources;

const LIMITS: [u64; 3] = [1_000_000_000, 1_000_000, 1_000_000_000];

fn invoke(
    source: &sources::Source,
    multiple: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let literal = source.omitted_score.map_or(Value::Null, Value::Int32);
    let mut score = CreateColumn::new("Score", ScalarType::Int32);
    if source.expression.is_some() {
        score = score.with_default_literal(&literal)?;
    }
    if let Some(required) = source.required {
        score = score.with_required(required);
    }
    let columns = [CreateColumn::new("Id", ScalarType::Int32), score];
    let rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
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

#[test]
fn complete_creation_defaults_require_measured_exact_pools_and_reject_one_less() {
    let mut contexts = 0;
    for case in [
        sources::Case::Absent,
        sources::Case::Minimum,
        sources::Case::Null,
        sources::Case::RequiredNull,
    ] {
        let source = sources::source(case);
        for multiple in [false, true] {
            let mut measured = ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2]);
            assert_eq!(
                invoke(&source, multiple, &mut measured, &|| false)
                    .unwrap()
                    .as_bytes(),
                source.bytes
            );
            for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
                .into_iter()
                .enumerate()
            {
                let used = LIMITS[position] - measured.remaining(resource);
                assert!(used > 0);
                let mut exact = LIMITS;
                exact[position] = used;
                let mut exact = ResourceBudget::new(exact[0], exact[1], exact[2]);
                assert_eq!(
                    invoke(&source, multiple, &mut exact, &|| false)
                        .unwrap()
                        .as_bytes(),
                    source.bytes
                );
                let mut short = LIMITS;
                short[position] = used - 1;
                let mut short = ResourceBudget::new(short[0], short[1], short[2]);
                assert!(matches!(invoke(&source, multiple, &mut short, &|| false),
                    Err(CreateDatabaseError::Inspect(InspectError::Limit(failure))) if failure.resource == resource));
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 8);
}

#[test]
fn complete_creation_defaults_poll_entry_middle_and_actual_terminal_cancellation() {
    let mut contexts = 0;
    for case in [
        sources::Case::Absent,
        sources::Case::Minimum,
        sources::Case::Null,
        sources::Case::RequiredNull,
    ] {
        let source = sources::source(case);
        for multiple in [false, true] {
            let polls = Cell::new(0);
            let mut budget = ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2]);
            let created = invoke(&source, multiple, &mut budget, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            assert_eq!(created.as_bytes(), source.bytes);
            let total = polls.get();
            assert!(total > 2);
            for stop in [1, total / 2, total] {
                polls.set(0);
                let mut budget = ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2]);
                let result = invoke(&source, multiple, &mut budget, &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                });
                assert!(matches!(
                    result,
                    Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
                ));
                assert_eq!(polls.get(), stop);
            }
            contexts += 1;
        }
    }
    assert_eq!(contexts, 8);
}
