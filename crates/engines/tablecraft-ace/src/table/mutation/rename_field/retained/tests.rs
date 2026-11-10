//! Independent full-image names, retained rows and original request boundaries.
use super::*;
use crate::{InspectError, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        CASES, readdition::Target,
    },
    saved_field_rename::fitting::current_history as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(2515);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 30_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn retained_field_rename_matches_all_independent_images_and_preserves_native_identities() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let before = raw::source(case, target, filled);
                let lowered = target.name().to_ascii_lowercase();
                for name in ["CurrentRevenueField", "X", lowered.as_str(), target.name()] {
                    let expected = raw::rename(&before, target.name(), name);
                    assert!(
                        image(&before)
                            .select_native_retained_field_rename_source(
                                "Table1",
                                target.name(),
                                &mut budget(),
                                &|| false,
                            )
                            .unwrap()
                    );
                    let facts = image(&before)
                        .inspect_retained_field_rename_source(
                            "Table1",
                            target.name(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    let candidate = image(&before)
                        .prepare_retained_field_rename(
                            "Table1",
                            target.name(),
                            name,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap_or_else(|error| {
                            panic!("{case:?}/{target:?}/{filled}/{name}: {error:?}")
                        });
                    equal_image(candidate.before_bytes(), &before);
                    equal_image(candidate.after_bytes(), &expected);
                    assert_eq!(candidate.identity(), facts.identity());
                    assert_ne!(
                        candidate.identity().column_number(),
                        usize::from(candidate.identity().column_id())
                    );
                    assert_eq!(candidate.changed(), target.name() != name);
                    let observed = read_table(
                        &mut Cursor::new(candidate.after_bytes()),
                        SCOPE,
                        "Table1",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(observed.rows(), raw::expected(case, target, filled));
                }
                assert!(
                    image(&before)
                        .prepare_current_field_rename(
                            "Table1",
                            target.name(),
                            "ClosedOldLanguage",
                            &mut budget(),
                            &|| false,
                        )
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn retained_renames_repeat_without_moving_off_or_replaying_old_field_history() {
    for case in CASES {
        let before = raw::source(case, Target::ShortText, true);
        let expected_first = raw::rename(&before, Target::ShortText.name(), "Revenue");
        let first = image(&before)
            .prepare_retained_field_rename(
                "Table1",
                Target::ShortText.name(),
                "Revenue",
                &mut budget(),
                &|| false,
            )
            .unwrap();
        equal_image(first.after_bytes(), &expected_first);
        let expected_second = raw::rename(&expected_first, "Revenue", "Total");
        let second = image(first.after_bytes())
            .prepare_retained_field_rename("Table1", "Revenue", "Total", &mut budget(), &|| false)
            .unwrap();
        equal_image(second.after_bytes(), &expected_second);
    }
}

#[test]
fn retained_rename_checks_actual_dependencies_names_and_terminal_controls_even_for_noop() {
    let before = raw::source(CASES[0], Target::ShortText, true);
    for name in ["", "bad.name"] {
        assert!(matches!(
            image(&before).prepare_retained_field_rename(
                "Table1",
                Target::ShortText.name(),
                name,
                &mut budget(),
                &|| false,
            ),
            Err(Error::InvalidName)
        ));
    }
    assert!(matches!(
        image(&before).prepare_retained_field_rename(
            "Table1",
            Target::ShortText.name(),
            "雪",
            &mut budget(),
            &|| false,
        ),
        Err(Error::CurrentNameShape)
    ));
    assert!(matches!(
        image(&before).prepare_retained_field_rename(
            "Table1",
            Target::ShortText.name(),
            "iD",
            &mut budget(),
            &|| false,
        ),
        Err(Error::DuplicateName)
    ));
    assert!(matches!(
        image(&before).prepare_retained_field_rename(
            "Table1",
            Target::ShortText.name(),
            Target::ShortText.name(),
            &mut budget(),
            &|| true,
        ),
        Err(Error::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before).prepare_retained_field_rename(
            "Table1",
            Target::ShortText.name(),
            Target::ShortText.name(),
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(Error::Inspect(InspectError::Limit(_)))
    ));
}

#[test]
fn retained_rename_independent_proof_rejects_descriptor_and_inactive_history_corruption() {
    let before = raw::source(CASES[0], Target::ShortText, true);
    let source = source::Source::capture(
        image(&before),
        "Table1",
        Target::ShortText.name(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let body = build::properties(&source.observed, "Revenue", &mut budget(), &|| false).unwrap();
    let change = catalog_property::compose_retained_single_at_origin(
        &source.image,
        &source.observed.schema,
        body,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut after = build::definition(&source, "Revenue", &mut budget(), &|| false).unwrap();
    change.apply(&mut after, &mut budget(), &|| false).unwrap();
    verify::candidate(&source, &after, "Revenue", &change, &mut budget(), &|| {
        false
    })
    .unwrap();

    for offset in [
        source.observed.property_descriptor.start + 8,
        source.observed.property_body.start,
        8 * 4096 + 4095,
    ] {
        // NO_CLONE: each corruption owns a separate candidate while the exact
        // positive baseline remains available for the next independent refusal.
        let mut corrupt = after.clone();
        corrupt[offset] ^= 1;
        assert!(
            verify::candidate(
                &source,
                &corrupt,
                "Revenue",
                &change,
                &mut budget(),
                &|| false,
            )
            .is_err(),
            "forged byte {offset}"
        );
    }
}
