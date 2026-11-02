//! Frozen source images and literal TCNA4, exercising real coordinated planning/V2.
use super::{Storage, budget, resizable_storage::Target, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeCompleteInsertIntent, NativeImage, NativeRecoveryQualificationError, RowInsert,
    qualify_native_complete_insert_recovery,
};
use tablecraft_io::journal::{
    JournalError, RecoveryOutcome, inspect_image, prepare_resizable_expected,
};
use tablecraft_testkit::fixtures::access::genuine_wide_root_promotion::{self as sources, Case};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cases() -> Vec<Case> {
    let mut cases = sources::load_all(|raw, expected| assert_eq!(hash(raw), expected));
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(
            |raw, expected| assert_eq!(hash(raw), expected),
        ),
    );
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_leaf_compression::load_all(
            |raw, expected| assert_eq!(hash(raw), expected),
        ),
    );
    for case in &cases {
        case.assert_literal_layout();
    }
    cases
}

fn requests(case: &Case) -> Vec<RowInsert> {
    case.insert_values()
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn wire(case: &Case, id: i32) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    let binding: [u8; 32] = hash.finalize()[..].try_into().unwrap();
    case.wire_rows(binding, id)
}

fn intent(case: &Case, id: i32) -> NativeCompleteInsertIntent {
    NativeCompleteInsertIntent::decode(&wire(case, id), &mut budget(), &|| false).unwrap()
}

fn journal(case: &Case, after: &[u8], committed: bool) -> Storage {
    let mut target = Target::new(case.before.clone());
    let mut log = Storage::new(Vec::new());
    let prepared = prepare_resizable_expected(
        &mut target,
        &mut log,
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
    }
    log
}

#[test]
#[ignore = "requires six native/SDK-qualified simultaneous wider promotion source files"]
fn wider_promotions_commit_single_and_prefix_compressed_batch_tcna4_v2_images() {
    let cases = cases();
    let wires: Vec<_> = cases.iter().map(|case| wire(case, 401)).collect();
    let mut commits = 0;
    for (case, expected_wire) in cases.iter().zip(wires) {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                case.table,
                requests(case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), case.inserted);
        assert_eq!(plan.intent().table(), case.table);
        assert_eq!(plan.intent().rows().len(), case.inserted);
        for (row, values) in plan.intent().rows().iter().zip(case.insert_values()) {
            assert_eq!(row.values(), values);
        }
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected_wire
        );
        let mut target = Target::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            case.inserted
        );
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(&log.bytes.get_ref()[..8], b"TCIMG002");
        assert!(target.resizes > 0);
        let writes = target.storage.writes;
        let resizes = target.resizes;
        target.storage.fail_prefix = Some(0);
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_complete_insert_recovery(
            checked,
            intent(case, 401),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
        commits += 1;
    }
    assert_eq!(commits, 12);
}

#[test]
#[ignore = "requires literal four-page promotions; generic journals use only frozen images"]
fn wider_promotion_pending_partial_tails_and_true_mixed_old_pages_rollback_exactly() {
    let mut contexts = 0;
    for case in cases() {
        let length = case.before.len();
        let mut states = Vec::new();
        for tail in [0, 1, 4096, 8192, 12288, 16383, 16384] {
            let mut mixed = case.before.clone();
            mixed.extend_from_slice(&case.after[length..length + tail]);
            states.push(mixed);
        }
        // Original-prefix mixtures are independently specified page copies, not
        // a failed producer's intermediate target or a complete replacement.
        for pages in [
            vec![case.definition],
            vec![case.data[2]],
            vec![case.roots[1]],
            vec![case.roots[1], case.data[2]],
            vec![case.roots[1], case.roots[2], case.definition],
        ] {
            let mut mixed = case.before.clone();
            for page in pages {
                mixed[page * 4096..(page + 1) * 4096]
                    .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
            }
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            states.push(mixed);
        }
        let mut map_states = Vec::new();
        for originally_set in [true, false] {
            let mut offsets = std::collections::BTreeSet::new();
            for &(offset, mask) in case.claims {
                let before = case.before[offset] & mask != 0;
                let after = case.after[offset] & mask != 0;
                assert_ne!(before, after);
                if before == originally_set {
                    offsets.insert(offset);
                }
            }
            assert!(!offsets.is_empty());
            let mut mixed = case.before.clone();
            for offset in offsets {
                mixed[offset] = case.after[offset];
            }
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            assert!(!states.contains(&mixed));
            map_states.push(mixed);
        }
        assert_eq!(map_states.len(), 2);
        assert_ne!(map_states[0], map_states[1]);
        states.extend(map_states);
        assert_eq!(states.len(), 14);
        for initial in states {
            let mut target = Target::new(initial);
            let mut log = journal(&case, &case.after, false);
            let saved = log.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case, 401),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    RecoveryOutcome::RolledBack
                );
                assert_eq!(target.storage.bytes.get_ref(), &case.before);
            }
            let effects = (target.storage.writes, target.resizes);
            target.storage.fail_prefix = Some(0);
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_complete_insert_recovery(
                checked,
                intent(&case, 401),
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
            assert_eq!((target.storage.writes, target.resizes), effects);
            assert_eq!(log.bytes.get_ref(), &saved);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 168);
}

#[test]
#[ignore = "requires frozen coordinated promotion sources and original request wires"]
fn wider_promotion_committed_mixtures_and_forged_children_maps_histories_are_effect_free() {
    let mut committed_refusals = 0;
    let mut forgeries = 0;
    for case in cases() {
        let mut mixed = case.before.clone();
        mixed[case.roots[1] * 4096..(case.roots[1] + 1) * 4096]
            .copy_from_slice(&case.after[case.roots[1] * 4096..(case.roots[1] + 1) * 4096]);
        mixed.extend_from_slice(&case.after[case.before.len()..]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for confirm in [false, true] {
            let mut target = Target::new(mixed.clone());
            let mut log = journal(&case, &case.after, true);
            let saved = log.bytes.get_ref().clone();
            let effects = (target.storage.writes, target.resizes);
            target.storage.fail_prefix = Some(0);
            target.storage.fail_sync = true;
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_complete_insert_recovery(
                checked,
                intent(&case, 401),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let result = if confirm {
                qualified.confirm_completed(&mut target, &mut budget(), &|| false)
            } else {
                qualified.recover(&mut target, &mut budget(), &|| false)
            };
            assert!(matches!(result, Err(JournalError::TargetMismatch)));
            assert_eq!((target.storage.writes, target.resizes), effects);
            assert_eq!(target.storage.bytes.get_ref(), &mixed);
            assert!(
                target.storage.fail_sync,
                "Committed mismatch must not sync target"
            );
            assert_eq!(log.bytes.get_ref(), &saved);
            committed_refusals += 1;
        }
        for offset in case.forgery_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let mut log = journal(&case, &after, false);
            let saved = log.bytes.get_ref().clone();
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            assert!(
                matches!(
                    qualify_native_complete_insert_recovery(
                        checked,
                        intent(&case, 401),
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ),
                "{} h{} byte{offset}",
                case.table,
                case.header
            );
            assert_eq!(log.bytes.get_ref(), &saved);
            forgeries += 1;
        }
        let mut log = journal(&case, &case.after, false);
        let saved = log.bytes.get_ref().clone();
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_complete_insert_recovery(
                checked,
                intent(&case, 402),
                scope(),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(log.bytes.get_ref(), &saved);
    }
    assert_eq!(committed_refusals, 24);
    assert!(forgeries >= 240);
}

#[path = "wide_root_promotion/resources.rs"]
mod resources;

#[test]
#[ignore = "requires qualified original sources for stale binding and allocation controls"]
fn wider_promotions_preserve_fixed_legacy_boundaries_and_refuse_stale_originals_or_invalid_allocation_authority()
 {
    use tablecraft_ace::{EditError, NativeIntentError, RecoverableEditError};
    let mut contexts = 0;
    let mut duplicate_contexts = 0;
    for case in cases() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                case.table,
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let mut fixed = Storage::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert!(matches!(
            plan.prepare(&mut fixed, &mut log, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!(fixed.bytes.get_ref(), &case.before);
        assert!(log.bytes.get_ref().is_empty());
        assert_eq!(fixed.writes, 0);
        assert_eq!(log.writes, 0);

        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    requests(&case),
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        if case.inserted == 1 {
            let mut duplicate = case.values();
            duplicate[0] = tablecraft_core::values::Value::Int32(1);
            let error = image(&case.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    case.table,
                    vec![RowInsert::new(duplicate)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Inspect(
                    tablecraft_ace::InspectError::IndexConstraint(
                        tablecraft_ace::IndexConstraintViolation::DuplicateKey
                    )
                ))
            ));
            duplicate_contexts += 1;
        }

        // Historical MAX/MAX two-row refusal belonged to the removed prefix0
        // policy. Its request is not the qualified MAX/MAX-1 batch oracle.
        // Duplicate primary keys, fixed-only APIs, stale binding and invalid
        // original map claims remain active admission controls below.

        let mut changed_before = case.before.clone();
        let marker = case.data[0] * 4096 + 4096 - 19 + 14;
        changed_before[marker] ^= 1;
        let stale = NativeCompleteInsertIntent::new(
            case.table,
            requests(&case),
            &changed_before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut log = journal(&case, &case.after, false);
        let saved = log.bytes.get_ref().clone();
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_complete_insert_recovery(
                checked,
                stale,
                scope(),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::Intent(
                NativeIntentError::BeforeMismatch
            ))
        ));
        assert_eq!(log.bytes.get_ref(), &saved);
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                case.table,
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let mut target = Target::new(changed_before.clone());
        let mut log = Storage::new(Vec::new());
        assert!(matches!(
            plan.prepare_insert(&mut target, &mut log, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!(target.storage.bytes.get_ref(), &changed_before);
        assert_eq!(
            (target.storage.writes, target.resizes, log.writes),
            (0, 0, 0)
        );
        assert!(log.bytes.get_ref().is_empty());

        for index in [0, 1] {
            let mut bad = case.before.clone();
            let (offset, mask) = case.claims[index];
            bad[offset] ^= mask;
            // Sequential rows remain unchanged; that alone is not write authority.
            let read = tablecraft_ace::read_table(
                &mut Cursor::new(&bad),
                scope(),
                case.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), case.rows(false));
            assert!(
                image(&bad)
                    .plan_recoverable_ordinary_complete_row_inserts(
                        case.table,
                        requests(&case),
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
        contexts += 1;
    }
    assert_eq!(contexts, 12);
    assert_eq!(duplicate_contexts, 4);
}
