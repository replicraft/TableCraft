//! Map-owned source selection, separate from complete selected admission.
use super::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{self as number, mixed},
    canonical_database_options, database_properties, image as ace12, mutation_fixture,
};

const SCOPE: CatalogScope = CatalogScope::new(976);
const PAGE: usize = 4096;
const MAP_PAGE: usize = 3;
const MAP_DIRECTORY: usize = 14;
const PROPERTY_OWNED_SLOT: usize = 16;
const MAP_HEADER: usize = 5;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn purpose(bytes: &[u8]) -> DatabaseOptionsSourcePurpose {
    image(bytes)
        .disable_name_auto_correct_source_purpose(&mut budget(), &|| false)
        .unwrap()
}

#[test]
fn actual_empty_or_allocated_column_ledger_selects_once_without_user_carrier_guessing() {
    for source in [number::Source::IdOnly, number::Source::GuiRequiredZero] {
        let before = number::source(source);
        assert_eq!(purpose(&before), DatabaseOptionsSourcePurpose::Creator);
    }
    for before in [
        number::repeated::source(number::Source::IdOnly),
        mixed::append_text(
            &mixed::source(mixed::SourceCase::IdOnly),
            "Label",
            255,
            true,
        ),
    ] {
        assert_eq!(purpose(&before), DatabaseOptionsSourcePurpose::FieldHistory);
    }
    // User LvProp is absent, but the database role owns the actual column row.
    let off = canonical_database_options::explicit_off(&number::source(number::Source::IdOnly));
    assert_eq!(purpose(&off), DatabaseOptionsSourcePurpose::FieldHistory);
    assert!(matches!(
        image(&off).check_disable_name_auto_correct_for_source(&mut budget(), &|| false),
        Err(DatabaseOptionsSourceError::Source)
    ));
}

#[test]
fn new_selected_boundary_qualifies_old_and_new_sources_without_widening_old_check() {
    let creator = number::source(number::Source::IdOnly);
    let external = number::repeated::source(number::Source::IdOnly);
    assert_eq!(
        image(&creator)
            .check_disable_name_auto_correct_for_source(&mut budget(), &|| false)
            .unwrap(),
        DatabaseOptionsSourcePurpose::Creator
    );
    assert_eq!(
        image(&external)
            .check_disable_name_auto_correct_for_source(&mut budget(), &|| false)
            .unwrap(),
        DatabaseOptionsSourcePurpose::FieldHistory
    );
    assert!(
        image(&external)
            .check_disable_name_auto_correct_source(&mut budget(), &|| false)
            .is_err()
    );
    let composed = canonical_database_options::append_explicit_off_to_owned_page(&external);
    assert_eq!(
        purpose(&composed),
        DatabaseOptionsSourcePurpose::FieldHistory
    );
    assert!(matches!(
        image(&composed).check_disable_name_auto_correct_for_source(&mut budget(), &|| false),
        Err(DatabaseOptionsSourceError::Source)
    ));
}

#[test]
fn twelve_page_history_keeps_field_purpose_then_refuses_current_eleven_page_source() {
    let mut before = number::repeated::source(number::Source::IdOnly);
    for index in 2..=14 {
        before = number::repeated::append(&before, &format!("N{index}"));
    }
    assert_eq!(before.len(), 12 * PAGE);
    assert_eq!(purpose(&before), DatabaseOptionsSourcePurpose::FieldHistory);
    assert!(matches!(
        image(&before).check_disable_name_auto_correct_for_source(&mut budget(), &|| false),
        Err(DatabaseOptionsSourceError::Source)
    ));
}

#[test]
fn actual_role_overlap_remains_typed_storage_failure_not_unavailability_or_retry() {
    let mut before = number::repeated::source(number::Source::IdOnly);
    let slot = MAP_PAGE * PAGE + MAP_DIRECTORY + PROPERTY_OWNED_SLOT * 2;
    let start = u16::from_le_bytes(before[slot..slot + 2].try_into().unwrap()) as usize;
    // The property ownership map now aliases the real ordinary catalog page4.
    before[MAP_PAGE * PAGE + start + MAP_HEADER] |= 1 << 4;
    assert!(matches!(
        image(&before).disable_name_auto_correct_source_purpose(&mut budget(), &|| false),
        Err(DatabaseOptionsSourceError::Storage(EditError::Unsupported(
            _
        )))
    ));
}

#[test]
fn old_profiles_cancellation_and_limits_remain_distinct_before_metadata_work() {
    for before in [mutation_fixture::database(), ace12(false, false)] {
        assert!(matches!(
            image(&before).disable_name_auto_correct_source_purpose(&mut budget(), &|| false),
            Err(DatabaseOptionsSourceError::Source)
        ));
    }
    let before = number::repeated::source(number::Source::IdOnly);
    assert!(matches!(
        image(&before).disable_name_auto_correct_source_purpose(&mut budget(), &|| true),
        Err(DatabaseOptionsSourceError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&before)
            .disable_name_auto_correct_source_purpose(&mut ResourceBudget::new(0, 0, 0), &|| false),
        Err(DatabaseOptionsSourceError::Inspect(_))
    ));
}

#[test]
fn selected_creator_proof_retains_all_original_sources_and_opaque_complements() {
    for mut before in super::super::tests::originals() {
        before[4 * PAGE + 512..4 * PAGE + 528].copy_from_slice(b"catalog-sentinel");
        before[3 * PAGE + 512..3 * PAGE + 528].copy_from_slice(b"map-gap-sentinel");
        before[7 * PAGE + 3500..7 * PAGE + 3516].copy_from_slice(b"retained-opaque!");
        assert_eq!(purpose(&before), DatabaseOptionsSourcePurpose::Creator);
        assert_eq!(
            image(&before)
                .check_disable_name_auto_correct_for_source(&mut budget(), &|| false)
                .unwrap(),
            DatabaseOptionsSourcePurpose::Creator
        );
    }
}

#[test]
fn source_eligibility_rejects_actual_nonfitting_history_without_an_after() {
    let mut before = number::repeated::source(number::Source::IdOnly);
    for index in 2..=12 {
        before = number::repeated::append(&before, &format!("N{index}"));
    }
    assert_eq!(before.len(), 11 * PAGE);
    assert_eq!(
        u16::from_le_bytes(before[10 * PAGE + 2..10 * PAGE + 4].try_into().unwrap()),
        596
    );
    assert_eq!(
        image(&before)
            .check_disable_name_auto_correct_for_source(&mut budget(), &|| false)
            .unwrap(),
        DatabaseOptionsSourcePurpose::FieldHistory
    );
    let nonfitting = number::repeated::append(&before, "N13");
    assert_eq!(nonfitting.len(), 11 * PAGE);
    assert_eq!(
        u16::from_le_bytes(nonfitting[10 * PAGE + 2..10 * PAGE + 4].try_into().unwrap()),
        71
    );
    assert_eq!(
        purpose(&nonfitting),
        DatabaseOptionsSourcePurpose::FieldHistory
    );
    assert!(matches!(
        image(&nonfitting).check_disable_name_auto_correct_for_source(&mut budget(), &|| false),
        Err(DatabaseOptionsSourceError::Source)
    ));
}

#[test]
fn common_ineligible_catalog_preserves_absent_and_other_read_facts() {
    let other = database_properties::name_auto_correct_values(&[(1, 4, &1_i32.to_le_bytes())]);
    for (before, expected) in [
        (
            database_properties::database(None),
            crate::DatabaseNameAutoCorrect::Absent,
        ),
        (
            database_properties::database(Some(&other)),
            crate::DatabaseNameAutoCorrect::Other,
        ),
    ] {
        let native = image(&before);
        let observed = crate::inspect_database_properties(
            &mut native.reader(),
            native.catalog().catalog().scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            observed
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            expected
        );
        assert!(matches!(
            native.check_disable_name_auto_correct_for_source(&mut budget(), &|| false),
            Err(DatabaseOptionsSourceError::Source)
        ));
    }
}
