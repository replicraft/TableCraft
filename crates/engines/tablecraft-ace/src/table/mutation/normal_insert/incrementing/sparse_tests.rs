//! Independent whole images and original-presence proof for sparse LONG fields.
use super::proof;
use crate::{
    EditError, InsertFieldValue, InspectError, NativeEditPlan, NativeImage,
    NativeOmittedInsertIntent, OmittedRowInsert, read_table,
};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::{self as raw, inserting},
    put16,
};

const SCOPE: CatalogScope = CatalogScope::new(439);
const STEP_LIMIT: u64 = 5_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, STEP_LIMIT)
}

fn image(before: &[u8], work: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, work, &|| false).unwrap()
}

fn source() -> Vec<u8> {
    let before = raw::source(raw::SourceCase::NumberHistory);
    raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved)
}

fn requests(inputs: Vec<InsertFieldValue>) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(inputs)]
}

fn omitted(count: usize) -> Vec<OmittedRowInsert> {
    requests((0..count).map(|_| InsertFieldValue::Omitted).collect())
}

fn observed_rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    let mut work = budget();
    let image = image(bytes, &mut work);
    let snapshot = read_table(&mut image.reader(), SCOPE, "Table1", &mut work, &|| false).unwrap();
    snapshot.rows().to_vec()
}

#[test]
fn observed_gapped_source_omission_null_and_supplied_value_match_independent_complete_images() {
    let before = source();
    for (input, expected) in [
        (InsertFieldValue::Omitted, Some(0)),
        (InsertFieldValue::Supplied(Value::Null), None),
        (InsertFieldValue::Supplied(Value::Int32(37)), Some(37)),
    ] {
        let originals = requests(vec![InsertFieldValue::Omitted, input]);
        let after = inserting::first_number_row(&before, &[(0, Some(1)), (2, expected)]);
        if expected == Some(0) {
            let hash = Sha256::digest(&after)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(
                hash,
                "b451db719c7735c626fe58bd2f3e39530a4dbb03c2bda1d39a4c5ceb26618c30"
            );
        }
        let mut work = budget();
        let plan = image(&before, &mut work)
            .plan_ordinary_omitted_row_inserts("Table1", &originals, &mut work, &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(
            observed_rows(&after),
            vec![vec![
                Value::Int32(1),
                expected.map_or(Value::Null, Value::Int32)
            ]]
        );
    }
}

#[test]
fn multiple_native_holes_and_a_second_mask_byte_preserve_offsets_defaults_and_counter_history() {
    let before = inserting::number_source(9, &[1, 3, 7]);
    let numbers = [0, 2, 4, 5, 6, 8, 9];
    let values = numbers
        .iter()
        .map(|&number| (number, Some(i32::from(number == 0))))
        .collect::<Vec<_>>();
    let after = inserting::first_number_row(&before, &values);
    assert_eq!(&after[9 * 4096 - 2..9 * 4096], &[0x75, 0x03]);
    let mut work = budget();
    let plan = image(&before, &mut work)
        .plan_ordinary_omitted_row_inserts("Table1", &omitted(numbers.len()), &mut work, &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), after);
    let mut work = budget();
    let next = image(&after, &mut work)
        .plan_ordinary_omitted_row_inserts("Table1", &omitted(numbers.len()), &mut work, &|| false)
        .unwrap();
    let rows = observed_rows(next.after_bytes());
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        vec![
            Value::Int32(1),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0)
        ]
    );
    assert_eq!(
        rows[1],
        vec![
            Value::Int32(2),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0),
            Value::Int32(0)
        ]
    );
    assert_eq!(&next.after_bytes()[10 * 4096..], &before[10 * 4096..]);
}

#[test]
fn generated_native_number_and_stored_ids_are_not_compacted_vector_positions() {
    let before = inserting::shifted_pair(&source());
    let expected = inserting::first_number_row(&before, &[(2, Some(1)), (9, Some(0))]);
    assert_eq!(&expected[9 * 4096 - 2..9 * 4096], &[0x04, 0x02]);
    let mut work = budget();
    let plan = image(&before, &mut work)
        .plan_ordinary_omitted_row_inserts("Table1", &omitted(2), &mut work, &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(
        observed_rows(&expected),
        vec![vec![Value::Int32(1), Value::Int32(0)]]
    );
}

#[test]
fn empty_sparse_insert_preserves_complete_source_checks_without_advancing_history() {
    let before = source();
    let mut work = budget();
    let plan = image(&before, &mut work)
        .plan_ordinary_omitted_row_inserts("Table1", &[], &mut work, &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), before);
    assert_eq!(plan.changed_rows(), 0);
    let mut foreign = before;
    foreign[7 * 4096 + 90] = 3;
    let mut work = budget();
    assert!(
        image(&foreign, &mut work)
            .plan_ordinary_omitted_row_inserts("Table1", &[], &mut work, &|| false)
            .is_err()
    );
}

#[test]
fn independent_proof_rejects_hole_mask_padding_counter_key_and_unrelated_property_changes() {
    let before = source();
    let after = inserting::first_number_row(&before, &[(0, Some(1)), (2, Some(0))]);
    let originals = omitted(2);
    let plan = NativeEditPlan {
        before: before.clone().into(),
        after: after.clone(),
        changed_rows: 1,
    };
    proof::prove(&plan, SCOPE, "Table1", &originals, &mut budget(), &|| false).unwrap();
    for offset in [
        7 * 4096 + 20,
        7 * 4096 + 67,
        9 * 4096 - 9,
        9 * 4096 - 1,
        9 * 4096 + 480,
        11 * 4096 - 1,
    ] {
        let mut forged = after.clone();
        forged[offset] ^= 1;
        let plan = NativeEditPlan {
            before: before.clone().into(),
            after: forged,
            changed_rows: 1,
        };
        assert!(
            proof::prove(&plan, SCOPE, "Table1", &originals, &mut budget(), &|| false).is_err(),
            "tamper offset {offset}"
        );
    }
}

#[test]
fn unqualified_native_numbers_stored_ids_overlaps_and_variable_shapes_refuse_without_output() {
    let before = source();
    for (offset, value) in [
        (7 * 4096 + 105, 0),
        (7 * 4096 + 105, 3),
        (7 * 4096 + 109, 0),
        (7 * 4096 + 109, 65535),
        (7 * 4096 + 121, 0),
        (7 * 4096 + 41, 2),
        (7 * 4096 + 43, 1),
    ] {
        let mut foreign = before.clone();
        put16(&mut foreign, offset, value);
        let mut work = budget();
        let captured = image(&foreign, &mut work);
        assert!(
            captured
                .plan_ordinary_omitted_row_inserts("Table1", &omitted(2), &mut work, &|| false)
                .is_err(),
            "unqualified word at {offset}"
        );
        assert_eq!(before, source());
    }
    let mut foreign = before;
    foreign[7 * 4096 + 90] = 3;
    let mut work = budget();
    assert!(
        image(&foreign, &mut work)
            .plan_ordinary_omitted_row_inserts("Table1", &omitted(2), &mut work, &|| false)
            .is_err()
    );
}

#[test]
fn actual_deleted_text_variable_ordinal_gap_remains_a_separate_refused_geometry() {
    let before = raw::source(raw::SourceCase::MixedTextHistory);
    let before = raw::remove_nonprimary(&before, "Label", raw::Variant::AppendedGroupRemoved);
    let mut work = budget();
    let captured = image(&before, &mut work);
    assert!(matches!(
        captured.plan_ordinary_omitted_row_inserts("Table1", &omitted(3), &mut work, &|| false),
        Err(EditError::Unsupported(crate::WriteBlocker::ActiveContent))
    ));
}

#[test]
fn original_omission_intent_requalification_remains_exact_without_new_wire_or_permissions() {
    let before = source();
    let expected = inserting::first_number_row(&before, &[(0, Some(1)), (2, Some(0))]);
    let mut work = budget();
    let plan = image(&before, &mut work)
        .plan_recoverable_ordinary_omitted_row_inserts("Table1", omitted(2), &mut work, &|| false)
        .unwrap();
    let record = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(&record[..8], b"TCNA0002");
    let decoded = NativeOmittedInsertIntent::decode(&record, &mut budget(), &|| false).unwrap();
    decoded
        .verify_before(&before, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(decoded.rows().len(), 1);
    assert!(
        decoded.rows()[0]
            .values()
            .iter()
            .all(|input| matches!(input, InsertFieldValue::Omitted))
    );
    let fresh = image(&before, &mut budget());
    let plan = plan
        .requalify_with_budgets(fresh, &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        record
    );
}

#[test]
fn entry_middle_terminal_cancellation_and_native_quota_preserve_control_failures() {
    let before = source();
    let captured = image(&before, &mut budget());
    let calls = Cell::new(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        false
    };
    let mut work = budget();
    captured
        .plan_ordinary_omitted_row_inserts("Table1", &omitted(2), &mut work, &cancel)
        .unwrap();
    let checks = calls.get();
    assert!(checks > 2);
    for stop in [1, checks / 2, checks] {
        let captured = image(&before, &mut budget());
        let calls = Cell::new(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        assert!(matches!(
            captured.plan_ordinary_omitted_row_inserts(
                "Table1",
                &omitted(2),
                &mut budget(),
                &cancel
            ),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
    }
    let used = STEP_LIMIT - work.remaining(Resource::Steps);
    let captured = image(&before, &mut budget());
    let mut limited = ResourceBudget::new(512 * 1024 * 1024, 100_000, used - 1);
    assert!(matches!(
        captured.plan_ordinary_omitted_row_inserts("Table1", &omitted(2), &mut limited, &|| false),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
}
