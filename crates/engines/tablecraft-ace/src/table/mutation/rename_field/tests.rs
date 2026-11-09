use super::*;
use crate::{InspectError, NativeImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::{Source, mixed, repeated},
        canonical_database_options,
    },
    support::bytes::hex,
};

const PAGE: usize = 4096;
const SCOPE: CatalogScope = CatalogScope::new(985);

struct Case {
    before: Vec<u8>,
    old: &'static str,
    new: &'static str,
    pin: &'static str,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn cases() -> Vec<Case> {
    let number = repeated::source(Source::IdOnly);
    let second = repeated::append(&number, "Score2");
    let inline_text = mixed::append_text(
        &mixed::source(mixed::SourceCase::InlineNumber),
        "Label",
        255,
        true,
    );
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let text_number = mixed::append_number(&text, "AfterLabel");
    let text_number_text = mixed::append_text(&text_number, "OtherLabel", 255, true);
    [
        (
            number,
            "Score",
            "Total",
            "235a57e709bb65bc6a1d0cc6483c2b204f1d3b78924108ff20e96a6671ba263d",
        ),
        (
            second,
            "Score",
            "Total",
            "361b12800d168eff4260d715d6053c611d6773210a6e84dd4dcabbd932e91212",
        ),
        (
            inline_text,
            "Score",
            "Total",
            "b91b069f96057640704a41df1091b4b7de13b7647c44e297746d55050fbbfaa4",
        ),
        (
            text_number_text,
            "Label",
            "Title",
            "7604ae8af88cbcaf1be537e03cfc65c6f8095ed20a8f4c1113359c7d176ddfdc",
        ),
    ]
    .into_iter()
    .map(|(before, old, new, pin)| Case {
        before: canonical_database_options::append_explicit_off_to_owned_page(&before),
        old,
        new,
        pin,
    })
    .collect()
}

#[test]
fn four_native_candidates_match_independent_raw_after_hashes_and_reader_observations() {
    for case in cases() {
        let identity = image(&case.before)
            .inspect_same_width_field_rename_source("Table1", case.old, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(identity.column_number(), 1);
        assert_eq!(identity.column_id(), 1);
        assert_eq!(identity.name_utf16_units(), 5);
        let plan = image(&case.before)
            .prepare_same_width_field_rename("Table1", case.old, case.new, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.identity(), identity);
        assert_eq!(plan.table(), "Table1");
        assert_eq!(plan.column(), case.old);
        assert_eq!(plan.new_name(), case.new);
        assert!(plan.changed());
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(hex(&Sha256::digest(plan.after_bytes())), case.pin);
        assert_eq!(plan.after_bytes().len(), case.before.len());
        for page in [0, 1, 2, 3, 4, 5, 6, 8, 9] {
            assert_eq!(
                &plan.after_bytes()[page * PAGE..(page + 1) * PAGE],
                &case.before[page * PAGE..(page + 1) * PAGE]
            );
        }
    }
}

#[test]
fn no_op_and_case_only_names_keep_identity_and_complete_source_qualification() {
    let case = cases().remove(1);
    let same = image(&case.before)
        .prepare_same_width_field_rename("Table1", "Score", "Score", &mut budget(), &|| false)
        .unwrap();
    assert!(!same.changed());
    assert_eq!(same.before_bytes(), same.after_bytes());
    let lower = image(&case.before)
        .prepare_same_width_field_rename("Table1", "Score", "score", &mut budget(), &|| false)
        .unwrap();
    assert!(lower.changed());
    assert_eq!(lower.identity(), same.identity());
    let mut changed = case.before;
    changed[7 * PAGE + 3000] = 1;
    assert!(matches!(
        image(&changed).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "Score",
            &mut budget(),
            &|| false,
        ),
        Err(FieldRenameError::SourceProof(_))
    ));
}

#[test]
fn primary_fields_wrong_shapes_duplicate_names_and_missing_off_do_not_publish() {
    let case = cases().remove(2);
    assert!(matches!(
        image(&case.before).prepare_same_width_field_rename(
            "Table1",
            "ID",
            "Id",
            &mut budget(),
            &|| false,
        ),
        Err(FieldRenameError::Source)
    ));
    assert!(matches!(
        image(&case.before).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "Label",
            &mut budget(),
            &|| false,
        ),
        Err(FieldRenameError::DuplicateName)
    ));
    for replacement in ["", " bad!", "Scor!", "A\nBcd"] {
        assert!(matches!(
            image(&case.before).prepare_same_width_field_rename(
                "Table1",
                "Score",
                replacement,
                &mut budget(),
                &|| false,
            ),
            Err(FieldRenameError::InvalidName)
        ));
    }
    for replacement in ["Longer", "Scoré"] {
        assert!(matches!(
            image(&case.before).prepare_same_width_field_rename(
                "Table1",
                "Score",
                replacement,
                &mut budget(),
                &|| false,
            ),
            Err(FieldRenameError::NameShape)
        ));
    }
    let narrow = mixed::append_text(&mixed::source(mixed::SourceCase::IdOnly), "Label", 80, true);
    let narrow_off = canonical_database_options::append_explicit_off_to_owned_page(&narrow);
    assert!(matches!(
        image(&narrow_off).inspect_same_width_field_rename_source(
            "Table1",
            "Label",
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameError::Source)
    ));
    let missing = repeated::source(Source::IdOnly);
    assert!(matches!(
        image(&missing).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "Total",
            &mut budget(),
            &|| false,
        ),
        Err(FieldRenameError::SourceProof(_))
    ));
}

#[test]
fn candidate_verifier_protects_history_options_identity_and_the_two_live_names() {
    let case = cases().remove(3);
    let plan = image(&case.before)
        .prepare_same_width_field_rename("Table1", case.old, case.new, &mut budget(), &|| false)
        .unwrap();
    for offset in [
        3 * PAGE + 512,
        4 * PAGE + 512,
        7 * PAGE + 3000,
        11 * PAGE - 1,
        plan.source.observed.definition_entry.start + 9,
    ] {
        let mut changed = plan.after_bytes().to_vec();
        changed[offset] ^= 1;
        assert!(matches!(
            verify::candidate(&plan.source, &changed, case.new, &mut budget(), &|| false),
            Err(FieldRenameError::Preservation)
        ));
    }
    let mut missing_live_group = plan.after_bytes().to_vec();
    let group = plan.source.observed.property_name.clone();
    missing_live_group[group.clone()].copy_from_slice(&plan.before_bytes()[group]);
    assert!(matches!(
        verify::candidate(
            &plan.source,
            &missing_live_group,
            case.new,
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameError::Preservation)
    ));
}

#[test]
fn native_name_source_inspection_and_candidate_keep_cancellation_and_pool_limits() {
    let case = cases().remove(0);
    let actual = image(&case.before);
    assert!(matches!(
        actual.inspect_same_width_field_rename_source("Table1", "Score", &mut budget(), &|| true),
        Err(FieldRenameError::SourceProof(
            crate::FieldHistoryDatabaseOptionsError::Inspect(InspectError::Cancelled,)
        ))
    ));
    assert!(matches!(
        image(&case.before).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "Total",
            &mut budget(),
            &|| true,
        ),
        Err(FieldRenameError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        image(&case.before).prepare_same_width_field_rename(
            "Table1",
            "Score",
            "Total",
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(FieldRenameError::Inspect(InspectError::Limit(_)))
    ));
    assert_eq!(actual.as_bytes(), case.before);
}
