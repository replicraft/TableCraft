//! Complete independent byte oracles and explicit old-language refusal.
use super::*;
use crate::{
    check_table,
    format::{
        reader::{row_count, row_range},
        row::reference_slot,
    },
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::{Source, mixed::after_off as raw, repeated},
        canonical_database_options,
    },
    support::bytes::hex,
};

const SCOPE: CatalogScope = CatalogScope::new(1093);
const PAGE: usize = 4096;
const TABLE: &str = "Table1";

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn text() -> FieldAdditionTarget {
    FieldAdditionTarget::ShortText {
        max_utf16_units: 255,
        unicode_compression: true,
    }
}

#[test]
fn four_complete_fitting_candidates_match_raw_oracles_and_frozen_sdk_hashes() {
    for (case, number_pin, text_pin) in [
        (
            raw::SourceCase::Number,
            "fbcc7a8ab6fa4d10df64c376c80b99ca76d1200d64d8d4a5d31d7b3702cc7680",
            "0ebbef86bf96f5079a2b666ab375f0da2d220322d3feb124f7c94e17d6280240",
        ),
        (
            raw::SourceCase::Mixed,
            "9011e9cbe157a529a5c7e362d6555abc6f13c68179ab2e2cb59ddb2d36c15e66",
            "d5940a703ae582f27958b4369c8cdca97d9f566dd7cb79c63c4a39e72edf4002",
        ),
    ] {
        let before = raw::source(case);
        for (column, target, expected, pin) in [
            (
                "AfterOff",
                FieldAdditionTarget::NumberLongZero,
                raw::append_number(&before, "AfterOff"),
                number_pin,
            ),
            (
                "LabelOff",
                text(),
                raw::append_text(&before, "LabelOff"),
                text_pin,
            ),
        ] {
            let candidate = image(&before)
                .prepare_created_add_field_after_off_candidate(
                    TABLE,
                    column,
                    target,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.before_bytes(), before);
            assert_eq!(candidate.after_bytes(), expected);
            assert_eq!(hex(&Sha256::digest(candidate.after_bytes())), pin);
            assert_eq!(candidate.table(), TABLE);
            assert_eq!(candidate.column(), column);
            assert_eq!(candidate.target(), target);
            let old_columns =
                u16::from_le_bytes(before[7 * PAGE + 45..7 * PAGE + 47].try_into().unwrap())
                    as usize;
            assert_eq!(candidate.column_count(), old_columns + 1);
            assert_eq!(
                &candidate.after_bytes()[3 * PAGE..4 * PAGE],
                &before[3 * PAGE..4 * PAGE]
            );
            assert_eq!(
                &candidate.after_bytes()[9 * PAGE..10 * PAGE],
                &before[9 * PAGE..10 * PAGE]
            );
            assert!(
                image(&before)
                    .prepare_created_add_field_candidate(
                        TABLE,
                        column,
                        target,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
            // The database ROW is now nonlast: the original settled source
            // language is deliberately not widened to accept this later event.
            assert!(
                image(candidate.after_bytes())
                    .check_field_history_name_auto_correct_off_source(&mut budget(), &|| false)
                    .is_err()
            );
            let old_rows = row_count(&before[10 * PAGE..11 * PAGE]).unwrap();
            let after = image(candidate.after_bytes());
            let ledger = catalog_property::ledger::capture(
                after.as_bytes(),
                after.catalog(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let object = after
                .catalog()
                .origins()
                .iter()
                .find(|origin| origin.native_id() == 7)
                .unwrap()
                .object_id();
            let (reference, _) = ledger
                .single_reference(object, &mut budget(), &|| false)
                .unwrap()
                .unwrap();
            assert_eq!(usize::from(reference_slot(reference)), old_rows);
        }
    }
}

#[test]
fn unqualified_source_target_duplicate_and_later_event_inputs_never_publish_candidates() {
    let before = raw::source(raw::SourceCase::Number);
    for target in [
        FieldAdditionTarget::ShortText {
            max_utf16_units: 80,
            unicode_compression: true,
        },
        FieldAdditionTarget::ShortText {
            max_utf16_units: 255,
            unicode_compression: false,
        },
    ] {
        assert!(matches!(
            image(&before).prepare_created_add_field_after_off_candidate(
                TABLE,
                "Other",
                target,
                &mut budget(),
                &|| false
            ),
            Err(FieldAdditionAfterOffError::Target)
        ));
    }
    assert!(
        image(&before)
            .prepare_created_add_field_after_off_candidate(
                TABLE,
                "Score",
                FieldAdditionTarget::NumberLongZero,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let missing = repeated::source(Source::IdOnly);
    assert!(matches!(
        image(&missing).prepare_created_add_field_after_off_candidate(
            TABLE,
            "Other",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false
        ),
        Err(FieldAdditionAfterOffError::SourceProof(_))
    ));
    let later = raw::append_number(&before, "AfterOff");
    assert!(matches!(
        image(&later).prepare_created_add_field_after_off_candidate(
            TABLE,
            "Again",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| false
        ),
        Err(FieldAdditionAfterOffError::SourceProof(_))
    ));
}

#[test]
fn actual_source_complement_off_settings_cancellation_and_limits_stay_checked() {
    let before = raw::source(raw::SourceCase::Mixed);
    for offset in [7 * PAGE + 3000, 3 * PAGE + 512] {
        let mut changed = before.clone();
        changed[offset] ^= 1;
        assert!(
            image(&changed)
                .prepare_created_add_field_after_off_candidate(
                    TABLE,
                    "Other",
                    FieldAdditionTarget::NumberLongZero,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
    }
    assert!(matches!(
        image(&before).prepare_created_add_field_after_off_candidate(
            TABLE,
            "Other",
            FieldAdditionTarget::NumberLongZero,
            &mut budget(),
            &|| true
        ),
        Err(FieldAdditionAfterOffError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        image(&before)
            .prepare_created_add_field_after_off_candidate(
                TABLE,
                "Other",
                FieldAdditionTarget::NumberLongZero,
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}

#[test]
fn fitted_only_owner_refuses_growth_without_changing_any_source_bytes() {
    let mut fields = repeated::source(Source::IdOnly);
    for index in 2..=12 {
        fields = repeated::append(&fields, &format!("N{index}"));
    }
    let before = canonical_database_options::append_explicit_off_to_owned_page(&fields);
    let original = image(&before);
    let checked = check_table(&mut original.reader(), SCOPE, TABLE, &mut budget(), &|| {
        false
    })
    .unwrap();
    // N12 left 596 bytes; explicit OFF consumed 154. A new complete 533-byte
    // Number user body cannot fit. Probe physical placement with no authority.
    assert_eq!(
        u16::from_le_bytes(before[10 * PAGE + 2..10 * PAGE + 4].try_into().unwrap()),
        442
    );
    assert!(matches!(
        catalog_property::compose_fitted_at_origin(
            &original,
            checked.schema(),
            vec![0; 533],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(_))
    ));
    assert_eq!(original.as_bytes(), before);
}

#[test]
fn exact_after_guards_reject_database_catalog_and_retired_row_tampering() {
    let before = raw::source(raw::SourceCase::Mixed);
    let expected = raw::append_text(&before, "LabelOff");
    let source = source::Source::capture(image(&before), TABLE, &mut budget(), &|| false).unwrap();
    let old_rows = row_count(&before[10 * PAGE..11 * PAGE]).unwrap();
    let (_, body_start, body_end) = row_range(&expected[10 * PAGE..11 * PAGE], old_rows).unwrap();
    let body = expected[10 * PAGE + body_start..10 * PAGE + body_end].to_vec();
    // Bind the actual physical append plan to the independent fixture body;
    // this is private verification plumbing, not a public source capability.
    let change = catalog_property::compose_fitted_at_origin(
        source.proof.image(),
        source.checked.schema(),
        body,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let check_after = |after: &[u8]| -> Result<(), FieldAdditionAfterOffError> {
        let mut work = budget();
        property_preservation::repeated(
            source.proof.image(),
            after,
            &source.checked,
            "LabelOff",
            &change,
            &mut work,
            &|| false,
        )
        .map_err(FieldAdditionError::from)?;
        verify::candidate(&source, after, &change, &mut work, &|| false)
    };
    check_after(&expected).unwrap();
    let catalog = &before[4 * PAGE..5 * PAGE];
    let last = row_count(catalog).unwrap() - 1;
    let (_, database_start, _) = row_range(catalog, last).unwrap();
    for offset in [
        4 * PAGE + database_start + 12,
        4 * PAGE + database_start + 48,
        3 * PAGE + 512,
        11 * PAGE - 1,
    ] {
        let mut changed = expected.clone();
        changed[offset] ^= 1;
        assert!(check_after(&changed).is_err());
    }
}

mod namespace;
