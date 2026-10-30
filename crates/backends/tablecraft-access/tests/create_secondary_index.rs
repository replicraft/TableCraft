#![cfg(feature = "ace16-write-preview")]
//! Actual Access SQL consumer over source-first independent schema expectations.
#[path = "create_secondary_index/source.rs"]
mod source;

use source::{budget, captured, same_bytes, sql};
use tablecraft_access::engineering::{self, EngineeringRequest, sql_index};
use tablecraft_ace::SecondaryIndexCreate;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access::secondary_index as model;

#[test]
fn qualify_create_secondary_index_host_sources_before_consumer() {
    source::qualify_all();
}

#[test]
fn sql_consumer_matches_all_sixteen_whole_images_and_native_metadata() {
    let mut count = 0;
    for code in [5, 6] {
        for population in model::POPULATIONS {
            for descending in [false, true] {
                let case = model::source(code, population, descending);
                let query = parse_query(&sql(&case), ParseLimits::default()).unwrap();
                let mut work = budget();
                let image = captured(&case.before, &mut work);
                let copy =
                    sql_index::preview_create_index(image, query.query(), &mut work, &|| false)
                        .unwrap();

                same_bytes(copy.before_bytes(), &case.before);
                same_bytes(copy.candidate_bytes(), &case.expected);
                assert_eq!(copy.affected_rows(), 0);
                assert!(copy.generated_values().is_none());
                let created = copy.created_index().unwrap();
                assert_eq!(created.name, case.name);
                assert_eq!(created.logical_index_number, 4);
                assert_eq!(created.physical_index_number, 2);
                assert_eq!(created.indexed_rows, case.rows.len());
                source::qualify(copy.candidate_bytes(), &case, true);
                count += 1;
            }
        }
    }
    assert_eq!(count, 16);
}

#[test]
fn typed_request_uses_the_same_copy_contract_and_native_refusals() {
    let case = model::source(6, model::Population::NullableDuplicate, false);
    let request = SecondaryIndexCreate::new(case.name, "Score", false);
    let copy = engineering::preview(
        captured(&case.before, &mut budget()),
        EngineeringRequest::CreateSecondaryIndex {
            table: "Items",
            request: &request,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    same_bytes(copy.candidate_bytes(), &case.expected);
    assert_eq!(copy.affected_rows(), 0);

    for (table, name, column) in [
        ("Items", "PrimaryAlias", "Score"),
        ("Items", "NewLookup", "Id"),
        ("Items", "NewLookup", "Absent"),
        ("Items", "1Bad", "Score"),
        ("Absent", "NewLookup", "Score"),
    ] {
        let request = SecondaryIndexCreate::new(name, column, false);
        assert!(
            engineering::preview(
                captured(&case.before, &mut budget()),
                EngineeringRequest::CreateSecondaryIndex {
                    table,
                    request: &request
                },
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn cumulative_capture_lowering_and_finish_budgets_and_cancellation_are_honest() {
    let case = model::source(6, model::Population::Distinct, false);
    let query = parse_query(&sql(&case), ParseLimits::default()).unwrap();
    let initial = [256 * 1024 * 1024, 100_000, 100_000_000];
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let run = |work: &mut ResourceBudget| {
        let image = captured(&case.before, work);
        sql_index::preview_create_index(image, query.query(), work, &|| false)
    };
    let mut measured = ResourceBudget::new(initial[0], initial[1], initial[2]);
    same_bytes(
        run(&mut measured).unwrap().candidate_bytes(),
        &case.expected,
    );
    let used = std::array::from_fn::<_, 3, _>(|position| {
        initial[position] - measured.remaining(pools[position])
    });
    assert!(used.iter().all(|value| *value > 0));
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    same_bytes(run(&mut exact).unwrap().candidate_bytes(), &case.expected);

    // Separate capture from the fallible closure so a one-below capture limit is
    // itself an expected typed refusal rather than a fixture-helper panic.
    for position in 0..3 {
        let mut limits = used;
        limits[position] -= 1;
        let mut short = ResourceBudget::new(limits[0], limits[1], limits[2]);
        let result = tablecraft_ace::NativeImage::capture(
            &mut std::io::Cursor::new(&case.before),
            source::scope(),
            &mut short,
            &|| false,
        )
        .map_err(sql_index::CreateIndexError::from)
        .and_then(|image| {
            sql_index::preview_create_index(image, query.query(), &mut short, &|| false)
        });
        assert!(result.is_err(), "one-below pool {position}");
    }

    let image = captured(&case.before, &mut budget());
    assert!(matches!(
        sql_index::preview_create_index(image, query.query(), &mut budget(), &|| true),
        Err(sql_index::CreateIndexError::Inspect(
            tablecraft_ace::InspectError::Cancelled
        ))
    ));
}
