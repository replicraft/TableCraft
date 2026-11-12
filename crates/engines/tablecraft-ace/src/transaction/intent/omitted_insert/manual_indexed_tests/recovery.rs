//! Fresh TCNA11 reconstruction owns DATA, key membership and historical counters.
use super::{Error, Intent, budget, literal};
use crate::{InsertFieldValue, NativeImage, OmittedRowInsert, RecoverableNativeOmittedInsertPlan};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::inserting as raw,
    support::io::journal::MemoryStorage,
};

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn rows(case: &raw::Case) -> Vec<Vec<Option<Value>>> {
    case.values
        .iter()
        .enumerate()
        .map(|(position, value)| {
            vec![if position == 0 && value.is_none() {
                None
            } else {
                Some(value.map_or(Value::Null, Value::Int32))
            }]
        })
        .collect()
}
fn requests(case: &raw::Case) -> Vec<OmittedRowInsert> {
    rows(case)
        .into_iter()
        .map(|fields| {
            OmittedRowInsert::new(
                fields
                    .into_iter()
                    .map(|field| {
                        field.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            )
        })
        .collect()
}
fn wire(case: &raw::Case) -> Vec<u8> {
    literal(
        &case.updated.expected,
        case.updated.creation.table,
        rows(case),
    )
}
fn decoded(case: &raw::Case) -> Intent {
    Intent::decode_retained_manual_indexed(&wire(case), &mut budget(), &|| false).unwrap()
}
fn plan(case: &raw::Case) -> RecoverableNativeOmittedInsertPlan {
    image(&case.updated.expected, 41120)
        .plan_recoverable_retained_manual_indexed_insert(
            case.updated.creation.table,
            requests(case),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing full image byte"
    );
}

#[test]
fn original_requalification_and_v3_states_reconstruct_complete_indexed_insert_images() {
    let mut empty = raw::source(false);
    empty.values.clear();
    empty.expected = empty.updated.expected.to_vec();
    for case in [
        raw::source(false),
        raw::source(true),
        raw::unique(false, false),
        raw::unique(true, true),
        empty,
    ] {
        let captured = plan(&case);
        assert!(captured.intent().is_retained_manual_indexed());
        assert_eq!(
            captured.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        same(captured.after_bytes(), &case.expected);
        let rebuilt = captured
            .requalify_with_budgets(
                image(&case.updated.expected, 41121),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(rebuilt.after_bytes(), &case.expected);
        for state in 0..3 {
            let mut target = MemoryStorage::new(case.updated.expected.to_vec());
            let mut file = MemoryStorage::new(Vec::new());
            let prepared = plan(&case)
                .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
                .unwrap();
            if state == 2 {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                if state == 1 {
                    *target.bytes.get_mut() = case.expected.to_vec();
                    let root = case.updated.creation.root as usize * 4096;
                    target.bytes.get_mut()[root..root + 4096]
                        .copy_from_slice(&case.updated.expected[root..root + 4096]);
                }
            }
            let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
            let qualified = decoded(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(41122),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                qualified.state(),
                if state == 2 {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            assert_eq!(
                qualified
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if state == 2 {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            same(
                target.bytes.get_ref(),
                if state == 2 {
                    &case.expected
                } else {
                    &case.updated.expected
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
            decoded(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(41123),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

fn reject(case: &raw::Case, original: &[u8], after: &[u8]) {
    let mut target = MemoryStorage::new(case.updated.expected.to_vec());
    let mut file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut file,
            &case.updated.expected,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = file.writes;
    let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
    assert!(
        Intent::decode_retained_manual_indexed(original, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(41124), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, file.writes), (0, writes));
    same(target.bytes.get_ref(), &case.updated.expected);
}

#[test]
fn wrong_source_request_after_and_constraint_intents_cannot_publish_a_partial_insert() {
    let case = raw::source(false);
    let mut wrong = wire(&case);
    wrong[24] ^= 1;
    reject(&case, &wrong, &case.expected);
    let wrong = literal(
        &case.updated.expected,
        case.updated.creation.table,
        vec![vec![Some(Value::Int32(100))]],
    );
    reject(&case, &wrong, &case.expected);
    for offset in [
        case.updated.creation.root as usize * 4096 + 490,
        case.updated.creation.target_definition as usize * 4096 + 3000,
        3 * 4096 + 3000,
    ] {
        let mut after = case.expected.to_vec();
        after[offset] ^= 1;
        reject(&case, &wire(&case), &after);
    }
    let mut stale = case.updated.expected.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case)
            .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, file.writes), (0, 0));
    assert!(
        plan(&case)
            .requalify_with_budgets(image(&stale, 41125), &mut budget(), &mut budget(), &|| {
                false
            })
            .is_err()
    );
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.updated.expected, 41126),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.updated.expected, 41127),
                &mut budget(),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    for required in [false, true] {
        let case = raw::unique(false, required);
        let duplicate = literal(
            &case.updated.expected,
            case.updated.creation.table,
            vec![vec![Some(Value::Int32(7))], vec![Some(Value::Int32(2))]],
        );
        reject(&case, &duplicate, &case.expected);
        if required {
            for row in [vec![None], vec![Some(Value::Null)]] {
                let original = literal(
                    &case.updated.expected,
                    case.updated.creation.table,
                    vec![row],
                );
                reject(&case, &original, &case.expected);
            }
        }
    }
}

#[test]
fn manual_indexed_original_cannot_prepare_or_qualify_a_v1_save() {
    use crate::{NativeRecoveryQualificationError, qualify_native_omitted_insert_recovery};
    use tablecraft_io::journal::JournalError;
    let case = raw::source(false);
    let mut target = MemoryStorage::new(case.updated.expected.to_vec());
    let mut file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan(&case).prepare(&mut target, &mut file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!((target.writes, file.writes), (0, 0));
    drop(
        journal::prepare_expected(
            &mut target,
            &mut file,
            &case.updated.expected,
            &case.expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = file.writes;
    let checked = journal::inspect_v1(&mut file, &mut budget(), &|| false).unwrap();
    let refused = qualify_native_omitted_insert_recovery(
        checked,
        decoded(&case),
        CatalogScope::new(41128),
        &mut budget(),
        &|| false,
    );
    assert!(matches!(
        refused,
        Err(NativeRecoveryQualificationError::Intent(
            Error::UnsupportedVersion
        ))
    ));
    assert_eq!((target.writes, file.writes), (0, writes));
    same(target.bytes.get_ref(), &case.updated.expected);
}

#[test]
fn compressed_guard_and_typed_companions_rebuild_from_the_original_under_v3() {
    for descending in [false, true] {
        let case = raw::compressed::source(descending);
        let original = literal(
            &case.updated,
            case.creation.table,
            vec![raw::compressed::values().into_iter().map(Some).collect()],
        );
        for committed in [false, true] {
            let request = OmittedRowInsert::new(
                raw::compressed::values()
                    .into_iter()
                    .map(InsertFieldValue::Supplied)
                    .collect(),
            );
            let captured = image(&case.updated, 41129)
                .plan_recoverable_retained_manual_indexed_insert(
                    case.creation.table,
                    vec![request],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(captured.after_bytes(), &case.expected);
            assert_eq!(
                captured.intent().encode(&mut budget(), &|| false).unwrap(),
                original
            );
            let mut target = MemoryStorage::new(case.updated.to_vec());
            let mut file = MemoryStorage::new(Vec::new());
            let prepared = captured
                .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                *target.bytes.get_mut() = case.expected.to_vec();
            }
            let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
            let qualified =
                Intent::decode_retained_manual_indexed(&original, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(41130),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
            let expected_outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            assert_eq!(
                qualified
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                expected_outcome
            );
            same(
                target.bytes.get_ref(),
                if committed {
                    &case.expected
                } else {
                    &case.updated
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
            Intent::decode_retained_manual_indexed(&original, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(41131),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}
