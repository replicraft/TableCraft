//! Complete independently authored shared-column AFTERs and closed refusals.
use super::*;
use crate::{
    format::{
        reader::{row_count, row_range},
        row::reference,
    },
    table::mutation::catalog_property::database_role::RoleAppend,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{Source as NumberSource, mixed, repeated},
    canonical_database_options,
};
use tablecraft_testkit::support::bytes::hex;

const SCOPE: CatalogScope = CatalogScope::new(970);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn sources() -> Vec<Vec<u8>> {
    let number = repeated::source(NumberSource::IdOnly);
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let inline = mixed::append_text(
        &mixed::source(mixed::SourceCase::InlineNumber),
        "Label",
        255,
        true,
    );
    let text_number = mixed::append_number(&text, "AfterLabel");
    let text_number_text = mixed::append_text(&text_number, "OtherLabel", 255, true);
    vec![
        number.clone(),
        repeated::append(&number, "Score2"),
        text,
        inline,
        text_number_text,
    ]
}

#[test]
fn five_complete_candidates_match_raw_images_and_sdk_pins_with_old_profiles_closed() {
    let pins = [
        "c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef",
        "6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620",
        "47daf2169c931ee2e6545f61234ae227d8c02b166fd91c78d43d0efff2eadcd3",
        "e30ac2923f754892895a97231cee7cc6bd8f880c03f384a21546ed4b7e4b77f3",
        "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
    ];
    for (before, pin) in sources().into_iter().zip(pins) {
        let expected = canonical_database_options::append_explicit_off_to_owned_page(&before);
        let plan = image(&before)
            .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(hex(&Sha256::digest(plan.after_bytes())), pin);
        assert_eq!(plan.after_bytes().len(), before.len());
        assert_eq!(
            &plan.after_bytes()[3 * PAGE..4 * PAGE],
            &before[3 * PAGE..4 * PAGE]
        );
        assert_eq!(
            &plan.after_bytes()[7 * PAGE..10 * PAGE],
            &before[7 * PAGE..10 * PAGE]
        );
        let old_property_count = row_count(&before[10 * PAGE..11 * PAGE]).unwrap();
        assert_eq!(
            plan.property_reference(),
            (10 << 8) | old_property_count as u32
        );
        assert!(
            image(&before)
                .plan_disable_name_auto_correct(&mut budget(), &|| false)
                .is_err()
        );
        assert!(
            image(plan.after_bytes())
                .check_created_field_addition_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            image(plan.after_bytes())
                .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
                .is_err()
        );
    }
}

fn placement(before: &[u8], after: &[u8]) -> RoleAppend {
    let catalog_count = row_count(&before[4 * PAGE..5 * PAGE]).unwrap();
    let property_count = row_count(&before[10 * PAGE..11 * PAGE]).unwrap();
    let (_, catalog_start, catalog_end) =
        row_range(&after[4 * PAGE..5 * PAGE], catalog_count).unwrap();
    let (_, property_start, property_end) =
        row_range(&after[10 * PAGE..11 * PAGE], property_count).unwrap();
    RoleAppend {
        catalog_bytes: after[4 * PAGE..5 * PAGE].to_vec(),
        property_bytes: after[10 * PAGE..11 * PAGE].to_vec(),
        catalog_range: 4 * PAGE + catalog_start..4 * PAGE + catalog_end,
        property_range: 10 * PAGE + property_start..10 * PAGE + property_end,
        root: reference(4, catalog_count as u8),
        property: reference(10, property_count as u8),
    }
}

#[test]
fn untouched_complement_and_native_option_body_tampering_refuse() {
    let before = sources().remove(4);
    let plan = image(&before)
        .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
        .unwrap();
    let original = image(&before);
    let source = source::capture(&original, &mut budget(), &|| false).unwrap();
    let placed = placement(&before, plan.after_bytes());
    verify::candidate(
        &original,
        plan.after_bytes(),
        &source,
        &placed,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    for offset in [
        512,
        3 * PAGE + 512,
        7 * PAGE + 3000,
        11 * PAGE - 1,
        placed.property_range.start,
    ] {
        let mut changed = plan.after_bytes().to_vec();
        changed[offset] ^= 1;
        assert!(
            verify::candidate(
                &original,
                &changed,
                &source,
                &placed,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}

#[test]
fn absent_history_foreign_complement_and_cancellation_never_publish_a_candidate() {
    let absent = mixed::source(mixed::SourceCase::IdOnly);
    assert!(matches!(
        image(&absent)
            .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false),
        Err(FieldHistoryDatabaseOptionsError::Source)
    ));
    let before = sources().remove(0);
    let mut changed = before.clone();
    changed[7 * PAGE + 3000] = 1;
    assert!(
        image(&changed)
            .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
            .is_err()
    );
    assert!(matches!(
        image(&before)
            .prepare_disable_name_auto_correct_after_field_history(&mut budget(), &|| true),
        Err(FieldHistoryDatabaseOptionsError::Inspect(
            InspectError::Cancelled
        ))
    ));
    let mut empty = ResourceBudget::new(0, 0, 0);
    assert!(
        image(&before)
            .prepare_disable_name_auto_correct_after_field_history(&mut empty, &|| false)
            .is_err()
    );
}
