//! Advisory write facts must not replace normal readable-source behavior.
use super::add_table_source_available;
use crate::{
    AddTableError, CreateColumn, CreateDatabaseError, FirstTableDefinition, InspectError,
    NativeImage, read_table,
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_add_table as fixtures, canonical_blank_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(493);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn definition() -> FirstTableDefinition {
    let id = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    FirstTableDefinition::capture(fixtures::NEW_TABLE, &[id], &mut budget(), &|| false).unwrap()
}

#[test]
fn qualified_populated_source_is_advisory_without_declaring_a_table_or_changing_opaque_bytes() {
    let fixture = fixtures::source();
    let source = image(&fixture.before);
    assert!(
        add_table_source_available(source.as_bytes(), SCOPE, &mut budget(), &|| false).unwrap()
    );
    assert_eq!(source.as_bytes(), fixture.before);
    assert_eq!(source.catalog().catalog().objects().len(), 8);
    assert_eq!(
        read_table(
            &mut source.reader(),
            SCOPE,
            fixtures::FIRST_TABLE,
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        &[fixture.original_row]
    );
}

#[test]
fn different_source_geometry_and_preexisting_eof_claims_report_unavailable() {
    let blank = image(&canonical_blank_creation::source());
    assert!(
        !add_table_source_available(blank.as_bytes(), SCOPE, &mut budget(), &|| false).unwrap()
    );
    for indexed in [false, true] {
        let source = image(&fixtures::claimed_append_destination(indexed));
        assert!(
            !add_table_source_available(source.as_bytes(), SCOPE, &mut budget(), &|| false)
                .unwrap()
        );
        assert!(matches!(
            source.plan_add_table(definition(), &mut budget(), &|| false),
            Err(AddTableError::Source)
        ));
    }
}

#[test]
fn unsupported_or_corrupt_primary_does_not_turn_an_advisory_fact_into_a_browse_failure() {
    for corrupt in [false, true] {
        let fixture = fixtures::source();
        let mut bytes = fixture.before;
        if corrupt {
            // The leaf still points at the actual row, but its encoded key differs.
            bytes[9 * 4096 + 484] = 2;
        } else {
            // Existing primary physical metadata gets an unsupported flag. The
            // sequential table reader does not use or silently repair this index.
            let physical = 75 + 2 * 25 + 6 + 12;
            bytes[7 * 4096 + physical + 46] |= 0x20;
        }
        let source = image(&bytes);
        assert_eq!(source.catalog().catalog().objects().len(), 8);
        assert_eq!(
            read_table(
                &mut source.reader(),
                SCOPE,
                fixtures::FIRST_TABLE,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            &[fixture.original_row]
        );
        assert!(
            !add_table_source_available(source.as_bytes(), SCOPE, &mut budget(), &|| false)
                .unwrap()
        );
        let refusal = source.plan_add_table(definition(), &mut budget(), &|| false);
        if corrupt {
            assert!(matches!(
                refusal,
                Err(AddTableError::Inspect(InspectError::Malformed))
            ));
        } else {
            assert!(matches!(refusal, Err(AddTableError::Source)));
        }
    }
}

fn resource_failure(result: Result<bool, AddTableError>) {
    assert!(matches!(
        result,
        Err(AddTableError::Inspect(InspectError::Limit(_)))
            | Err(AddTableError::Create(CreateDatabaseError::Inspect(
                InspectError::Limit(_)
            )))
    ));
}

#[test]
fn work_allocation_rows_and_cancellation_remain_errors_including_canonical_construction() {
    let source = image(&fixtures::source().before);
    let mut measured = budget();
    let initial = POOLS.map(|pool| measured.remaining(pool));
    let polls = Cell::new(0);
    assert!(
        add_table_source_available(source.as_bytes(), SCOPE, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap()
    );
    let used: [u64; 3] =
        std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|cost| *cost > 0));
    assert!(
        add_table_source_available(
            source.as_bytes(),
            SCOPE,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false
        )
        .unwrap()
    );
    for missing in 0..3 {
        let mut short = used;
        short[missing] -= 1;
        resource_failure(add_table_source_available(
            source.as_bytes(),
            SCOPE,
            &mut ResourceBudget::new(short[0], short[1], short[2]),
            &|| false,
        ));
    }
    let mut capture_budget = budget();
    let initial_capture_bytes = capture_budget.remaining(Resource::Bytes);
    NativeImage::capture(
        &mut Cursor::new(source.as_bytes()),
        SCOPE,
        &mut capture_budget,
        &|| false,
    )
    .unwrap();
    let capture_bytes = initial_capture_bytes - capture_budget.remaining(Resource::Bytes);
    assert!(matches!(
        add_table_source_available(
            source.as_bytes(),
            SCOPE,
            &mut ResourceBudget::new(capture_bytes, 100_000, 100_000_000),
            &|| false,
        ),
        Err(AddTableError::Create(CreateDatabaseError::Inspect(
            InspectError::Limit(_)
        )))
    ));
    for limit in [0, polls.get() / 2, polls.get() - 1] {
        let calls = Cell::new(0);
        let result = add_table_source_available(source.as_bytes(), SCOPE, &mut budget(), &|| {
            let at = calls.get();
            calls.set(at + 1);
            at >= limit
        });
        assert!(matches!(
            result,
            Err(AddTableError::Inspect(InspectError::Cancelled))
                | Err(AddTableError::Create(CreateDatabaseError::Inspect(
                    InspectError::Cancelled
                )))
        ));
    }
}

#[test]
fn large_unsupported_snapshot_returns_false_before_capture_or_allocation() {
    let captured = vec![0; 64 * 1024 * 1024];
    let mut work = ResourceBudget::new(0, 0, 1);
    assert!(!add_table_source_available(&captured, SCOPE, &mut work, &|| false).unwrap());
    assert_eq!(work.remaining(Resource::Bytes), 0);
    assert_eq!(work.remaining(Resource::Rows), 0);
    assert_eq!(work.remaining(Resource::Steps), 0);
    assert!(matches!(
        add_table_source_available(&captured, SCOPE, &mut ResourceBudget::new(0, 0, 0), &|| {
            false
        }),
        Err(AddTableError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        add_table_source_available(&captured, SCOPE, &mut ResourceBudget::new(0, 0, 1), &|| {
            true
        }),
        Err(AddTableError::Inspect(InspectError::Cancelled))
    ));
}
