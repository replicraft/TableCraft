//! Native V2 qualification precedes all source-backed recovery effects.
use super::{Storage, Target, budget, cases, scope};
use sha2::{Digest, Sha256};
use tablecraft_ace::{
    NativeRecoveryQualificationError, NativeUpdateIntent, qualify_native_allocation_recovery,
};
use tablecraft_io::journal::{
    JournalError, RecoveryOutcome, V2State, inspect_v2, prepare_resizable_expected,
};
use tablecraft_testkit::access::genuine_eof_allocation::Case;

fn text(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&[6, 0, 0, 0]);
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

// Independent original-only TCNI7 framing; no native encoder/planner is called.
fn literal_intent(case: &Case, replacement: &[u8]) -> Vec<u8> {
    let mut cell = Vec::new();
    cell.extend_from_slice(&0_u64.to_le_bytes());
    text(&mut cell, b"unchanged sentinel");
    text(&mut cell, replacement);
    let mut body = Vec::from(*b"Sentinel");
    for number in [(20 + 6 + 4 + cell.len()) as u32, 6, 1, 1, 0] {
        body.extend_from_slice(&number.to_le_bytes());
    }
    body.extend_from_slice(b"Marker");
    body.extend_from_slice(&((4 + cell.len()) as u32).to_le_bytes());
    body.extend_from_slice(&cell);
    let mut wire = Vec::from(*b"TCNI0007");
    wire.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    wire.extend_from_slice(&[7, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    wire.extend_from_slice(&digest.finalize());
    for number in [8_u32, 1, 1, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(&body);
    assert_eq!(wire.len(), 72 + body.len());
    assert_eq!(wire.len(), 171);
    wire
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(
        &literal_intent(case, b"aaaaaaaaaaaaaaaaaaa"),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

// Generic image-journal framing consumes only independently frozen images.
// The discarded fixture target is never a native expected-byte oracle.
fn journal(case: &Case, after: &[u8], committed: bool) -> Storage {
    let mut target = Target::new(case.before.clone());
    let mut journal = Storage::new(Vec::new());
    let prepared = prepare_resizable_expected(
        &mut target,
        &mut journal,
        &case.before,
        after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if committed {
        prepared.commit(&mut budget(), &|| false).unwrap();
    } else {
        drop(prepared);
        // A valid incomplete marker remains pending and must restore BEFORE.
        journal.bytes.get_mut().extend_from_slice(b"COM");
    }
    journal
}

#[test]
#[ignore = "requires all four independently qualified EOF allocation images"]
fn native_eof_pending_tail_and_mixed_pages_roll_back_truncate_repeat_and_confirm() {
    let mut contexts = 0;
    for case in cases() {
        let before_length = case.before.len();
        let mut states = Vec::new();
        for tail in [0, 1, 14, 4028, 4095, 4096] {
            let mut current = case.before.clone();
            current.extend_from_slice(&case.after[before_length..before_length + tail]);
            states.push(current);
        }
        let root = case.root as usize * 4096;
        let mut maps_only = case.after.clone();
        maps_only[root..root + 4096].copy_from_slice(&case.before[root..root + 4096]);
        states.push(maps_only);
        let mut root_only = case.before.clone();
        root_only[root..root + 4096].copy_from_slice(&case.after[root..root + 4096]);
        root_only.extend_from_slice(&case.after[before_length..]);
        states.push(root_only);
        for current in states {
            let mut target = Target::new(current.clone());
            let mut journal = journal(&case, &case.after, false);
            let retained_journal = journal.bytes.get_ref().clone();
            let journal_writes = journal.writes;
            for pass in 0..2 {
                let writes = target.storage.writes;
                let resizes = target.resizes;
                let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_allocation_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(qualified.state(), V2State::Pending);
                assert_eq!(target.storage.writes, writes);
                assert_eq!(target.resizes, resizes);
                assert_eq!(
                    target.storage.bytes.get_ref(),
                    if pass == 0 { &current } else { &case.before }
                );
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    RecoveryOutcome::RolledBack
                );
                assert_eq!(target.storage.bytes.get_ref(), &case.before);
                assert_eq!(target.storage.bytes.get_ref().len(), before_length);
            }
            let writes = target.storage.writes;
            let resizes = target.resizes;
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_allocation_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                RecoveryOutcome::RolledBack
            );
            assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
            assert_eq!(journal.bytes.get_ref(), &retained_journal);
            assert_eq!(journal.writes, journal_writes);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 16);
}

#[test]
#[ignore = "requires all four independently qualified EOF allocation images"]
fn native_eof_committed_mismatch_and_self_consistent_forgery_never_write_or_resize() {
    for case in cases() {
        for control in 0..2 {
            let current = if control == 0 {
                case.before.clone()
            } else {
                let mut bytes = case.after.clone();
                *bytes.last_mut().unwrap() ^= 1;
                bytes
            };
            let mut target = Target::new(current.clone());
            let mut journal = journal(&case, &case.after, true);
            let retained = journal.bytes.get_ref().clone();
            let writes = journal.writes;
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_allocation_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(qualified.state(), V2State::Committed);
            assert_eq!(
                qualified.recover(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            );
            assert_eq!(target.storage.bytes.get_ref(), &current);
            assert_eq!((target.storage.writes, target.resizes), (0, 0));
            assert_eq!(journal.bytes.get_ref(), &retained);
            assert_eq!(journal.writes, writes);
        }
        for control in 0..4 {
            let mut forged = case.after.clone();
            let root = case.root as usize * 4096;
            match control {
                0 => forged[root + 4034] ^= 1,
                1 => forged[case.before.len() + 4040] ^= 1,
                2 => {
                    let maps = if case.header == 5 { 108 } else { 126 };
                    let at = maps * 4096 + 3958 + 5 + case.root as usize / 8;
                    forged[at] &= !(1 << (case.root % 8));
                }
                3 => {}
                _ => unreachable!(),
            }
            let wire = if control == 3 {
                literal_intent(&case, b"bbbbbbbbbbbbbbbbbbb")
            } else {
                literal_intent(&case, b"aaaaaaaaaaaaaaaaaaa")
            };
            let target = Target::new(case.before.clone());
            let mut journal = journal(&case, &forged, false);
            let retained = journal.bytes.get_ref().clone();
            let writes = journal.writes;
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            let result = qualify_native_allocation_recovery(
                checked,
                NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap(),
                scope(),
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                result,
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            // The mismatch exists before any target was supplied to recovery.
            assert_eq!(target.storage.bytes.get_ref(), &case.before);
            assert_eq!((target.storage.writes, target.resizes), (0, 0));
            assert_eq!(journal.bytes.get_ref(), &retained);
            assert_eq!(journal.writes, writes);
        }
    }
}
