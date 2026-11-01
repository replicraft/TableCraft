//! Full public creator bounds, using measured costs and literal SOURCE bytes.
use super::super::CreatedDatabase;
use super::{
    CreateColumn, CreateDatabaseError, CreateTable, create_ace16_database,
    create_ace16_database_tables, sources,
};
use crate::{IndexConstraintViolation, InspectError};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const LIMITS: [u64; 3] = [1_000_000_000, 1_000_000, 1_000_000_000];

fn columns() -> [CreateColumn<'static>; 2] {
    [
        CreateColumn::new("Id", ScalarType::Int32)
            .with_primary_key("PrimaryId")
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32),
    ]
}

fn invoke(
    source: &sources::Source,
    multiple: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CreatedDatabase, CreateDatabaseError> {
    let columns = columns();
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
fn manual_primary_full_creators_require_measured_exact_pools_and_refuse_one_less() {
    let mut contexts = 0;
    for case in [
        sources::Case::Empty,
        sources::Case::SignedUnsorted,
        sources::Case::Boundary,
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
    assert_eq!(contexts, 6);
}

#[test]
fn manual_primary_full_creators_poll_entry_middle_and_actual_terminal_cancellation() {
    let mut contexts = 0;
    for case in [
        sources::Case::Empty,
        sources::Case::SignedUnsorted,
        sources::Case::Boundary,
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
    assert_eq!(contexts, 6);
}

#[test]
fn duplicate_and_null_keys_refuse_before_single_or_multi_image_allocation_even_when_late() {
    let healthy = sources::source(sources::Case::Boundary);
    let mut budget = ResourceBudget::new(LIMITS[0], LIMITS[1], LIMITS[2]);
    assert_eq!(
        invoke(&healthy, true, &mut budget, &|| false)
            .unwrap()
            .as_bytes(),
        healthy.bytes
    );
    let columns = columns();
    let healthy_rows = healthy.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut refused = 0;
    for case in [sources::Case::Duplicate, sources::Case::Null] {
        let source = sources::source(case);
        let expected = match source.violation.unwrap() {
            sources::Violation::DuplicateKey => IndexConstraintViolation::DuplicateKey,
            sources::Violation::NullKey => IndexConstraintViolation::NullKey,
        };
        let mut no_image = ResourceBudget::new(0, LIMITS[1], LIMITS[2]);
        assert!(matches!(invoke(&source, false, &mut no_image, &|| false),
                         Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected));
        refused += 1;

        // This independent file length cannot fund its complete image, but
        // leaves ample metadata budget without mirroring Request's layout cost.
        let bytes = u64::try_from(source.bytes.len()).unwrap() - 1;
        let mut no_image = ResourceBudget::new(bytes, LIMITS[1], LIMITS[2]);
        assert!(matches!(invoke(&source, true, &mut no_image, &|| false),
                         Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected));
        refused += 1;

        let bad_rows = source.rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let tables = [
            CreateTable::new("Items", &columns, &healthy_rows),
            CreateTable::new("Other", &columns, &bad_rows),
        ];
        let mut no_image = ResourceBudget::new(bytes, LIMITS[1], LIMITS[2]);
        assert!(
            matches!(create_ace16_database_tables(&tables, &mut no_image, &|| false),
                         Err(CreateDatabaseError::IndexConstraint(actual)) if actual == expected)
        );
        refused += 1;
    }
    assert_eq!(refused, 6);
}
