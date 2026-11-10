//! Current-source reconstruction retains original presence and exact journal images.
use crate::{
    InsertFieldValue, NativeImage, NativeOmittedInsertIntent, NativeRecoveryQualificationError,
    OmittedRowInsert, qualify_native_omitted_insert_recovery,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::mixed::after_off::{
            current_insert::{self as raw, Input},
            deleted_field::populated::{self as history, Case, readdition::Target},
        },
        tcna2,
    },
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn requests(case: Case, target: Target, input: Input) -> Vec<OmittedRowInsert> {
    let width = raw::row(case, target, Input::Omitted, 0).len();
    let mut fields = (0..width)
        .map(|_| InsertFieldValue::Omitted)
        .collect::<Vec<_>>();
    fields[width - 1] = match input {
        Input::Omitted => InsertFieldValue::Omitted,
        Input::Null => InsertFieldValue::Supplied(Value::Null),
        Input::Number123 => InsertFieldValue::Supplied(Value::Int32(123)),
        Input::Empty => InsertFieldValue::Supplied(Value::Text(String::new())),
        Input::Compressed => InsertFieldValue::Supplied(Value::Text("new label".into())),
        Input::Unicode => InsertFieldValue::Supplied(Value::Text("日本語🦀".into())),
    };
    vec![OmittedRowInsert::new(fields)]
}

fn literal(before: &[u8], case: Case, target: Target, input: Input) -> Vec<u8> {
    let width = raw::row(case, target, Input::Omitted, 0).len();
    let mut values = vec![None; width];
    values[width - 1] = match input {
        Input::Omitted => None,
        Input::Null => Some(Value::Null),
        Input::Number123 => Some(Value::Int32(123)),
        Input::Empty => Some(Value::Text(String::new())),
        Input::Compressed => Some(Value::Text("new label".into())),
        Input::Unicode => Some(Value::Text("日本語🦀".into())),
    };
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    let mut wire = tcna2::wider_presence_wire(
        "Table1",
        before.len(),
        digest.finalize().into(),
        vec![values],
    );
    wire[..8].copy_from_slice(b"TCNA0007");
    wire[12..14].copy_from_slice(&7_u16.to_le_bytes());
    wire
}

#[test]
fn current_insert_native_wire_and_fresh_scopes_reconstruct_original_presence() {
    for case in history::CASES {
        for filled in [false, true] {
            for renamed in [false, true] {
                let input = if filled {
                    Input::Unicode
                } else {
                    Input::Omitted
                };
                let before = raw::source(case, Target::ShortText, filled, renamed);
                let expected = raw::append(&before, case, Target::ShortText, input);
                let plan = image(&before, 2631)
                    .plan_recoverable_current_omitted_row_inserts(
                        "Table1",
                        requests(case, Target::ShortText, input),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                let wire = literal(&before, case, Target::ShortText, input);
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire
                );
                assert!(
                    NativeOmittedInsertIntent::decode(&wire, &mut budget(), &|| false).is_err()
                );
                let fresh = plan
                    .requalify_with_budgets(
                        image(&before, 2632),
                        &mut budget(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(fresh.before_bytes(), before);
                assert_eq!(fresh.after_bytes(), expected);
                assert_eq!(fresh.inserted_rows(), 1);
                assert!(
                    image(&before, 2631)
                        .plan_recoverable_ordinary_omitted_row_inserts(
                            "Table1",
                            requests(case, Target::ShortText, input),
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
fn current_insert_cold_recovery_handles_pending_and_committed_original_requests() {
    for case in history::CASES {
        let before = raw::source(case, Target::ShortText, true, true);
        let after = raw::append(&before, case, Target::ShortText, Input::Null);
        let wire = literal(&before, case, Target::ShortText, Input::Null);
        for committed in [false, true] {
            let plan = image(&before, 2631)
                .plan_recoverable_current_omitted_row_inserts(
                    "Table1",
                    requests(case, Target::ShortText, Input::Null),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut target = MemoryStorage::new(before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
            let intent =
                NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
            qualify_native_omitted_insert_recovery(
                checked,
                intent,
                CatalogScope::new(2633),
                &mut budget(),
                &|| false,
            )
            .unwrap()
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &before }
            );
        }
    }
}

#[test]
fn current_insert_recovery_refuses_unselected_legacy_sources_and_forged_after_bytes() {
    let case = Case::NumberPresent;
    let before = raw::source(case, Target::Number, true, true);
    assert!(
        !image(&before, 2631)
            .select_native_current_insert_source("Table1", &mut budget(), &|| false)
            .unwrap()
    );
    let after = raw::append(&before, case, Target::Number, Input::Omitted);
    let wire = literal(&before, case, Target::Number, Input::Omitted);
    let legacy = image(&before, 2631)
        .plan_recoverable_ordinary_omitted_row_inserts(
            "Table1",
            requests(case, Target::Number, Input::Omitted),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(legacy.after_bytes(), after);
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal_file,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
    let intent =
        NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_omitted_insert_recovery(
            checked,
            intent,
            CatalogScope::new(2631),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);

    let before = raw::source(Case::MixedBinary, Target::ShortText, true, true);
    let after = raw::append(
        &before,
        Case::MixedBinary,
        Target::ShortText,
        Input::Omitted,
    );
    let wire = literal(
        &before,
        Case::MixedBinary,
        Target::ShortText,
        Input::Omitted,
    );
    for offset in [
        7 * 4096 + 1000,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        10 * 4096 + 4090,
    ] {
        let mut forged = after.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut journal_file,
                &before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
        let intent =
            NativeOmittedInsertIntent::decode_current(&wire, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_omitted_insert_recovery(
                checked,
                intent,
                CatalogScope::new(2631),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
