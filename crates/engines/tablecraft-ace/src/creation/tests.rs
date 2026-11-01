//! Public creation acceptance against the independent canonical SOURCE owner.
use super::{CreateDatabaseError, create_ace16_int32_database};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_testkit::access::canonical_creation::{self, CASES, Case};
use tablecraft_testkit::access::canonical_native17_creation as native17;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

#[test]
fn creator_matches_all_six_complete_independent_sources() {
    for case in CASES {
        let mut source = canonical_creation::source(case);
        source.bytes = native17::source(source.bytes);
        let columns: Vec<_> = source.columns.iter().map(String::as_str).collect();
        let rows: Vec<_> = source.rows.iter().map(Vec::as_slice).collect();
        let created =
            create_ace16_int32_database(source.table, &columns, &rows, &mut budget(), &|| false)
                .unwrap();

        assert_eq!(created.as_bytes(), source.bytes, "case {}", case.id());
        assert_eq!(created.row_count(), source.rows.len());
        assert_eq!(created.column_count(), source.columns.len());
    }
}

#[test]
fn input_shape_limits_and_actual_page_fit_refuse_without_an_artifact() {
    assert!(matches!(
        create_ace16_int32_database("Items", &[], &[], &mut budget(), &|| false),
        Err(CreateDatabaseError::ColumnLimit)
    ));
    let seventeen: Vec<_> = (0..17).map(|index| format!("C{index}")).collect();
    let columns: Vec<_> = seventeen.iter().map(String::as_str).collect();
    assert!(matches!(
        create_ace16_int32_database("Items", &columns, &[], &mut budget(), &|| false),
        Err(CreateDatabaseError::ColumnLimit)
    ));

    let one = [7_i32];
    let sixty_five = [one.as_slice(); 65];
    assert!(matches!(
        create_ace16_int32_database("Items", &["Id"], &sixty_five, &mut budget(), &|| false),
        Err(CreateDatabaseError::RowLimit)
    ));
    assert!(matches!(
        create_ace16_int32_database("Items", &["Id", "Score"], &[&one], &mut budget(), &|| false),
        Err(CreateDatabaseError::RowWidth)
    ));
    assert!(matches!(
        create_ace16_int32_database("Items", &["Id"], &[&[]], &mut budget(), &|| false),
        Err(CreateDatabaseError::RowWidth)
    ));

    // The independent wide source has 58 * (68 + 2) + 14 used bytes. A 59th
    // row needs 70 bytes and cannot fit its 22 remaining bytes, despite being
    // below the separate 64-row boundary. Never silently split or truncate.
    let wide = canonical_creation::source(Case::Wide);
    let names: Vec<_> = wide.columns.iter().map(String::as_str).collect();
    let mut rows: Vec<_> = wide.rows.iter().map(Vec::as_slice).collect();
    rows.push(wide.rows[0].as_slice());
    assert!(matches!(
        create_ace16_int32_database(wide.table, &names, &rows, &mut budget(), &|| false),
        Err(CreateDatabaseError::PageCapacity)
    ));
}

#[test]
fn native_names_are_validated_without_coercion_before_allocation() {
    for name in [
        "",
        " bad",
        "bad\0name",
        "bad\nname",
        "bad\u{7f}name",
        "bad.name",
        "bad!name",
        "bad`name",
        "bad[name",
        "bad]name",
    ] {
        assert!(matches!(
            create_ace16_int32_database(name, &["Id"], &[], &mut budget(), &|| false),
            Err(CreateDatabaseError::InvalidName)
        ));
        assert!(matches!(
            create_ace16_int32_database("Items", &[name], &[], &mut budget(), &|| false),
            Err(CreateDatabaseError::InvalidName)
        ));
    }
    for name in ["MSysObjects", "msysrelationships", "MSYSACES"] {
        assert!(matches!(
            create_ace16_int32_database(name, &["Id"], &[], &mut budget(), &|| false),
            Err(CreateDatabaseError::SystemTableName)
        ));
    }
    let mut no_bytes = ResourceBudget::new(0, 100_000, 16_000_000);
    assert!(matches!(
        create_ace16_int32_database("Items", &["Score", "sCORE"], &[], &mut no_bytes, &|| false),
        Err(CreateDatabaseError::DuplicateColumn)
    ));
    assert_eq!(no_bytes.remaining(Resource::Bytes), 0);

    let too_many_units = "x".repeat(65);
    let astral_overflow = "🙂".repeat(33);
    for name in [&too_many_units, &astral_overflow] {
        assert!(matches!(
            create_ace16_int32_database(name, &["Id"], &[], &mut budget(), &|| false),
            Err(CreateDatabaseError::InvalidName)
        ));
    }
    // UTF-16 limits count code units, not UTF-8 bytes or Unicode scalars.
    // The admitted boundary also preserves spelling without normalization.
    let sixty_four_units = "🙂".repeat(32);
    let created = create_ace16_int32_database(
        &sixty_four_units,
        &[&sixty_four_units],
        &[],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(created.row_count(), 0);
    assert_eq!(created.column_count(), 1);
}

#[test]
fn each_independent_resource_pool_refuses_one_below_its_actual_charge() {
    let mut measured = budget();
    create_ace16_int32_database(
        "Items",
        &["Id", "Score"],
        &[&[1, 10]],
        &mut measured,
        &|| false,
    )
    .unwrap();
    let initial = budget();
    let used = [
        initial.remaining(Resource::Bytes) - measured.remaining(Resource::Bytes),
        initial.remaining(Resource::Rows) - measured.remaining(Resource::Rows),
        initial.remaining(Resource::Steps) - measured.remaining(Resource::Steps),
    ];
    for (position, resource) in [Resource::Bytes, Resource::Rows, Resource::Steps]
        .into_iter()
        .enumerate()
    {
        assert!(used[position] > 0);
        let mut pools = [16 * 1024 * 1024, 100_000, 16_000_000];
        pools[position] = used[position] - 1;
        let mut limited = ResourceBudget::new(pools[0], pools[1], pools[2]);
        let error = create_ace16_int32_database(
            "Items",
            &["Id", "Score"],
            &[&[1, 10]],
            &mut limited,
            &|| false,
        )
        .err()
        .expect("insufficient independent pool must refuse");
        match error {
            CreateDatabaseError::Inspect(InspectError::Limit(error)) => {
                assert_eq!(error.resource, resource);
            }
            other => panic!("unexpected typed resource refusal: {other:?}"),
        }

        pools[position] = used[position];
        let mut exact = ResourceBudget::new(pools[0], pools[1], pools[2]);
        let created = create_ace16_int32_database(
            "Items",
            &["Id", "Score"],
            &[&[1, 10]],
            &mut exact,
            &|| false,
        )
        .unwrap();
        assert_eq!(created.row_count(), 1);
        assert_eq!(exact.remaining(resource), 0);
    }
}

#[test]
fn cancellation_precedes_allocation_and_can_reject_the_completed_image() {
    let mut untouched = budget();
    let before_bytes = untouched.remaining(Resource::Bytes);
    assert!(matches!(
        create_ace16_int32_database("Items", &["Id"], &[], &mut untouched, &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(untouched.remaining(Resource::Bytes), before_bytes);

    let polls = Cell::new(0_usize);
    create_ace16_int32_database(
        "Items",
        &["Id", "Score"],
        &[&[1, 10]],
        &mut budget(),
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    let final_poll = polls.get();
    assert!(final_poll > 1);
    let observed = Cell::new(0_usize);
    assert!(matches!(
        create_ace16_int32_database(
            "Items",
            &["Id", "Score"],
            &[&[1, 10]],
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                observed.get() == final_poll
            }
        ),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(observed.get(), final_poll);
}
