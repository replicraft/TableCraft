//! Actual complete Source preflight and indexed empty-target refusals.
use crate::{NativeFixedIntentGroup, NativeImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_retained_numeric;

const SCOPE: CatalogScope = CatalogScope::new(136);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires both pinned genuine originals and all eight literal AFTERs"]
fn unchanged_wide_indexes_do_not_hide_unindexed_marker_during_capability_preflight() {
    let cases = genuine_retained_numeric::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    for case in cases {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        image
            .check_native_ordinary_fixed_table(case.table, &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("h{} {}: {error}", case.header, case.table));
        assert_eq!(image.as_bytes(), case.before);
    }
}

#[test]
#[ignore = "requires both pinned genuine originals and all eight literal AFTERs"]
fn retained_wide_indexed_empty_groups_still_refuse_before_any_mutation() {
    let cases = genuine_retained_numeric::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut refused = 0;
    for case in cases {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let groups = vec![
            NativeFixedIntentGroup::new("KeyValue", vec![], &mut budget(), &|| false).unwrap(),
        ];
        assert!(
            image
                .plan_recoverable_ordinary_fixed_updates(case.table, groups, &mut budget(), &|| {
                    false
                })
                .is_err()
        );
        // No Storage, journal, write or candidate supplies these expectations.
        // Empty groups qualify metadata rather than disappearing from intent.
        refused += 1;
    }
    assert_eq!(refused, 8);
}
