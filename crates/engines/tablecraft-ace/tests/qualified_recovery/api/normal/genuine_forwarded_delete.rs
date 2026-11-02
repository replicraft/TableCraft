//! Resolved original terminal witnesses; independent full images remain the oracle.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeDeleteIntent, NativeImage, NativeRecoveryQualificationError,
    RecoverableEditError, RowDelete, qualify_native_delete_recovery, read_table,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_forwarded_delete::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn facts(case: &Case) -> (usize, usize, usize) {
    match u32::from_le_bytes(case.before[20..24].try_into().unwrap()) {
        5 => (107, 110, 112),
        6 => (125, 127, 129),
        _ => panic!("unexpected pinned header"),
    }
}

fn raw(case: &Case) -> Vec<u8> {
    let (_, root, terminal) = facts(case);
    assert_eq!(
        &case.before[root * 4096 + 14..root * 4096 + 16],
        &[0xbe, 0x4f]
    );
    assert_eq!(
        u32::from_le_bytes(
            case.before[root * 4096 + 4030..root * 4096 + 4034]
                .try_into()
                .unwrap()
        ),
        (terminal as u32) << 8
    );
    let row = case.before[terminal * 4096 + 4028..terminal * 4096 + 4096].to_vec();
    assert_eq!(row.len(), 68);
    row
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn wire(case: &Case, expected: &[u8]) -> Vec<u8> {
    let mut result = Vec::from(*b"TCND0001");
    result.extend_from_slice(&((88 + expected.len()) as u32).to_le_bytes());
    result.extend_from_slice(&[1, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for value in [8_u32, 1, 0] {
        result.extend_from_slice(&value.to_le_bytes());
    }
    result.extend_from_slice(b"Sentinel");
    result.extend_from_slice(&0_u64.to_le_bytes());
    result.extend_from_slice(&(expected.len() as u32).to_le_bytes());
    result.extend_from_slice(expected);
    assert_eq!(result.len(), 88 + expected.len());
    result
}

fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, &raw(case)), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires all pinned EOF and forwarded DELETE SOURCE images"]
fn forwarded_delete_commits_literal_images_and_binds_terminal_not_pointer_capsule() {
    for case in cases() {
        let native = image(&case.before);
        let prepared = native
            .prepare_ordinary_row_deletes("Sentinel", &[0], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].expected_bytes(), raw(&case));
        let plan = native
            .plan_recoverable_ordinary_row_deletes("Sentinel", prepared, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case, &raw(&case))
        );
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.deleted_rows(), 1);
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        let table = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(table.rows().is_empty());
    }
}

fn replay(case: &Case, marker: Marker, current: Vec<u8>) {
    let committed = matches!(marker, Marker::Committed);
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let mut target = Storage::new(current);
    let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
    let retained = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let writes = target.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_delete_recovery(checked, intent(case), scope(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(target.writes, writes);
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            outcome
        );
        assert_eq!(
            target.bytes.get_ref(),
            if committed { &case.after } else { &case.before }
        );
    }
    if committed {
        assert_eq!(target.writes, 0);
    }
    let writes = target.writes;
    target.fail_prefix = Some(0);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified =
        qualify_native_delete_recovery(checked, intent(case), scope(), &mut budget(), &|| false)
            .unwrap();
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        outcome
    );
    assert_eq!(target.writes, writes);
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &retained);
}

#[test]
#[ignore = "requires all pinned EOF and forwarded DELETE SOURCE images"]
fn forwarded_delete_recovery_replays_full_and_causal_mixed_images_on_same_journal() {
    for case in cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            replay(&case, marker, case.after.clone());
        }
        let (definition, root, _) = facts(&case);
        for page in [definition, root] {
            let mut mixed = case.before.clone();
            mixed[page * 4096..(page + 1) * 4096]
                .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            for marker in [Marker::Pending, Marker::Partial] {
                replay(&case, marker, mixed.clone());
            }
            let mut target = Storage::new(mixed.clone());
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Committed,
            ));
            let retained = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_delete_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualified.recover(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            ));
            assert_eq!(target.bytes.get_ref(), &mixed);
            assert_eq!(target.writes, 0);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &retained);
        }
    }
}

#[test]
#[ignore = "requires all pinned EOF and forwarded DELETE SOURCE images"]
fn forwarded_delete_qualifier_rejects_protected_pointer_terminal_and_map_changes() {
    for case in cases() {
        let (definition, root, terminal) = facts(&case);
        for offset in [
            root * 4096 + 4030,
            root * 4096 + 4034,
            terminal * 4096 + 14,
            terminal * 4096 + 4030,
            terminal * 4096 + 4084,
            definition * 4096 + 20,
            4096 + 4027,
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let retained = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_delete_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &retained);
        }
        let mut expected = raw(&case);
        expected[6] ^= 1;
        let stale =
            NativeDeleteIntent::decode(&wire(&case, &expected), &mut budget(), &|| false).unwrap();
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let retained = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_delete_recovery(checked, stale, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::Edit(EditError::Conflict))
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &retained);
    }
}

#[test]
#[ignore = "requires all pinned EOF and forwarded DELETE SOURCE images"]
fn forwarded_delete_stale_terminal_and_pointer_witnesses_fail_before_a_plan() {
    for case in cases() {
        let (_, root, terminal) = facts(&case);
        let mut stale = raw(&case);
        stale[6] ^= 1;
        let capsule = case.before[root * 4096 + 4030..root * 4096 + 4096].to_vec();
        for expected in [stale, capsule] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_row_deletes(
                    "Sentinel",
                    vec![RowDelete::new(0, expected)],
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Conflict))
            ));
        }
        // A fresh healthy same-width terminal change cannot reuse an earlier
        // raw witness. No new AFTER is proposed for this refusal control.
        let mut changed = case.before.clone();
        changed[terminal * 4096 + 4034] = b'b';
        let native = image(&changed);
        let prepared = native
            .prepare_ordinary_row_deletes("Sentinel", &[0], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            prepared[0].expected_bytes(),
            &changed[terminal * 4096 + 4028..terminal * 4096 + 4096]
        );
        assert!(matches!(
            native.plan_recoverable_ordinary_row_deletes(
                "Sentinel",
                vec![RowDelete::new(0, raw(&case))],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
    }
}
