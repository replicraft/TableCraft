//! Independent row images retain the natural stale tail of a fitted rename.
use super::*;
use crate::{
    FixedColumnUpdates, FixedScalarUpdate, InsertFieldValue, NativeOmittedInsertIntent,
    NativeRecoveryQualificationError, OmittedRowInsert, RowDelete,
    qualify_native_omitted_insert_recovery, read_table,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, V1State, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting,
    canonical_initial_table_creation::deletes,
    saved_field_rename::fitting::{self, SourceCase},
};
use tablecraft_testkit::support::io::journal::MemoryStorage;

const SCOPE: CatalogScope = CatalogScope::new(991);
const DEFINITION: u32 = 7;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn shrunk() -> Vec<u8> {
    fitting::pair(SourceCase::NumberFirst, "X")[1].to_vec()
}

fn omission() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Omitted,
    ])]
}

fn row_image(empty: &[u8], value: i32) -> Vec<u8> {
    inserting::first_number_row(empty, &[(0, Some(1)), (1, Some(value))])
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first different byte: {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

#[test]
fn fitted_shrink_insert_update_and_delete_match_independent_full_images() {
    let empty = shrunk();
    let inserted = row_image(&empty, 0);
    let updated = row_image(&empty, 42);
    let deleted = deletes::only_long_row(&updated);
    let tail = 7 * 4096 + 239..8 * 4096;
    assert_eq!(
        &empty[7 * 4096 + 239..7 * 4096 + 247],
        &[75, 0, 101, 0, 121, 0, 255, 255]
    );

    let plan = image(&empty)
        .plan_ordinary_omitted_row_inserts("Table1", &omission(), &mut budget(), &|| false)
        .unwrap();
    assert_image(plan.after_bytes(), &inserted);

    let changes = [FixedScalarUpdate::new(0, Value::Int32(0), Value::Int32(42))];
    let groups = [FixedColumnUpdates::new("Table1", "X", &changes)];
    let plan = image(&inserted)
        .plan_ordinary_fixed_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    assert_image(plan.after_bytes(), &updated);

    let source = image(&updated);
    let requests = source
        .prepare_ordinary_row_deletes("Table1", &[0], &mut budget(), &|| false)
        .unwrap();
    let requests = [RowDelete::new(0, requests[0].expected_bytes().to_vec())];
    let plan = source
        .plan_ordinary_row_deletes("Table1", &requests, &mut budget(), &|| false)
        .unwrap();
    assert_image(plan.after_bytes(), &deleted);
    assert_eq!(&plan.after_bytes()[tail.clone()], &empty[tail]);
}

#[test]
fn original_omission_intent_reconstructs_fresh_tail_qualification() {
    let before = shrunk();
    let expected = row_image(&before, 0);
    let plan = image(&before)
        .plan_recoverable_ordinary_omitted_row_inserts("Table1", omission(), &mut budget(), &|| {
            false
        })
        .unwrap();
    let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&wire[..8], b"TCNA0002");
    let decoded = NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    let rebuilt = image(&before)
        .plan_ordinary_omitted_row_inserts(decoded.table(), decoded.rows(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_image(rebuilt.after_bytes(), &expected);
    let requalified = plan
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_image(requalified.after_bytes(), &expected);
    assert_eq!(
        requalified
            .intent()
            .encode(&mut budget(), &|| false)
            .unwrap(),
        wire
    );
}

#[test]
fn retained_witness_binds_source_definition_end_and_every_tail_byte() {
    let before = shrunk();
    let original = image(&before);
    let snapshot = read_table(
        &mut original.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let witness = super::super::validate_rows(
        &original,
        snapshot.schema(),
        super::super::RowDeclaration::SparseGenerated,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .unwrap();
    let after = row_image(&before, 0);
    witness
        .require_unchanged(&original, &after, DEFINITION, &mut budget(), &|| false)
        .unwrap();

    for offset in [7 * 4096 + 239, 8 * 4096 - 1, 7 * 4096 + 8] {
        let mut forged = after.clone();
        forged[offset] ^= 1;
        assert!(matches!(
            witness.require_unchanged(&original, &forged, DEFINITION, &mut budget(), &|| false,),
            Err(EditError::Preservation)
        ));
    }
    assert!(matches!(
        witness.require_unchanged(&image(&before), &after, DEFINITION, &mut budget(), &|| {
            false
        },),
        Err(EditError::Preservation)
    ));
    assert!(matches!(
        witness.require_unchanged(&original, &after, DEFINITION + 1, &mut budget(), &|| false,),
        Err(EditError::Preservation)
    ));
}

#[test]
fn nonzero_slack_remains_refused_for_global_roles_other_profiles_and_reserved_margin() {
    use super::super::{Role, raw};

    let before = shrunk();
    let original = image(&before);
    assert!(
        raw::capture(
            &original,
            DEFINITION,
            Role::SelectedGenerated,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    for role in [Role::User, Role::Catalog, Role::Relationships, Role::Aces] {
        assert!(raw::capture_rows(&original, DEFINITION, role, &mut budget(), &|| false).is_err());
    }

    // Use the actual catalog declaration as well as the row-role API guard.
    raw::capture(&original, 2, Role::Catalog, &mut budget(), &|| false).unwrap();
    let mut catalog_tail = before.clone();
    let catalog_length = u32::from_le_bytes(
        catalog_tail[2 * 4096 + 8..2 * 4096 + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    catalog_tail[2 * 4096 + catalog_length] = 1;
    assert!(
        raw::capture(
            &image(&catalog_tail),
            2,
            Role::Catalog,
            &mut budget(),
            &|| false
        )
        .is_err()
    );

    let mut foreign = before.clone();
    foreign[20] = 6;
    assert!(
        image(&foreign)
            .plan_ordinary_omitted_row_inserts("Table1", &omission(), &mut budget(), &|| false,)
            .is_err()
    );

    let mut margin = before;
    margin[8 * 4096 - 1] = 1;
    assert!(
        image(&margin)
            .plan_ordinary_omitted_row_inserts("Table1", &omission(), &mut budget(), &|| false,)
            .is_err()
    );
}

#[test]
fn cold_journal_reconstruction_qualifies_original_requests_and_rejects_forged_tail() {
    let before = shrunk();
    let after = row_image(&before, 0);
    for committed in [false, true] {
        let reviewed = image(&before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                "Table1",
                omission(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = reviewed
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut().copy_from_slice(&after);
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let decoded = NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_omitted_insert_recovery(checked, decoded, SCOPE, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(
            qualified.state(),
            if committed {
                V1State::Committed
            } else {
                V1State::Pending
            }
        );
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_image(
            target.bytes.get_ref(),
            if committed { &after } else { &before },
        );
    }

    let intent =
        NativeOmittedInsertIntent::new("Table1", omission(), &before, &mut budget(), &|| false)
            .unwrap();
    let mut forged = after;
    forged[7 * 4096 + 239] ^= 1;
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_omitted_insert_recovery(checked, intent, SCOPE, &mut budget(), &|| false,),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_image(target.bytes.get_ref(), &before);
    assert_eq!(target.writes, 0);
}
