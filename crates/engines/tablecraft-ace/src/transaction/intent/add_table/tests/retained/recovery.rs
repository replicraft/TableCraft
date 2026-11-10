//! The same checked V3 journal retains native qualification through cold recovery.
use super::{Added, budget, decode, definition, image, literal, sources};
use crate::{NativeAddTableIntent, qualify_native_add_table_recovery};
use sha2::{Digest, Sha256};
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table as raw, support::io::journal::MemoryStorage,
};

fn journal_literal(before: &[u8], after: &[u8], committed: bool) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCIMG003");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update((after.len() as u64).to_le_bytes());
    hash.update(before);
    hash.update(after);
    let digest = hash.finalize();
    let mut bytes = b"TCIMG003".to_vec();
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend((after.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&digest);
    bytes.extend(before);
    bytes.extend(after);
    if committed {
        bytes.extend(b"COMMIT03");
        bytes.extend_from_slice(&digest);
    }
    bytes
}

#[test]
fn retained_plans_encode_literals_and_recover_pending_or_committed_images_in_fresh_scopes() {
    for before in sources() {
        for target_kind in [Added::GeneratedNumber, Added::GeneratedText] {
            let expected = raw::append(&before, "More", target_kind);
            for committed in [false, true] {
                let plan = image(&before, 3051)
                    .plan_add_table(definition(target_kind), &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(plan.before_bytes(), before);
                assert_eq!(plan.after_bytes(), expected);
                assert_eq!(
                    plan.encode_intent(&mut budget(), &|| false).unwrap(),
                    literal(&before, target_kind)
                );
                let mut target = MemoryStorage::new(before.clone());
                let mut journal_file = MemoryStorage::new(Vec::new());
                let prepared = plan
                    .prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    // Complete candidate bytes are a legitimate interrupted target
                    // even though the durable journal has no commit marker yet.
                    target.bytes = std::io::Cursor::new(expected.clone());
                }
                assert_eq!(
                    journal_file.bytes.get_ref(),
                    &journal_literal(&before, &expected, committed)
                );
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let qualified = decode(&before, target_kind)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(3052),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(
                    qualified.state(),
                    if committed {
                        ResizableState::Committed
                    } else {
                        ResizableState::Pending
                    }
                );
                let expected_outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    expected_outcome
                );
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &expected } else { &before }
                );
                let writes = target.writes;
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let outcome = decode(&before, target_kind)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(3053),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(outcome, expected_outcome);
                assert_eq!(target.writes, writes);
            }
        }
    }
}

#[test]
fn forged_after_and_wrong_before_remain_data_without_native_recovery_authority() {
    let before = sources().into_iter().nth(1).unwrap();
    let expected = raw::append(&before, "More", Added::GeneratedText);
    for offset in [
        3 * 4096 + 3000,
        8 * 4096 + 4000,
        10 * 4096 + 4000,
        12 * 4096 + 4000,
    ] {
        let mut forged = expected.clone();
        forged[offset] ^= 1;
        let mut journal_file = MemoryStorage::new(journal_literal(&before, &forged, false));
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            decode(&before, Added::GeneratedText)
                .qualify_retained_recovery(checked, CatalogScope::new(3061), &mut budget(), &|| {
                    false
                })
                .is_err()
        );
        assert_eq!(journal_file.writes, 0);
    }
    let mut changed = before.clone();
    *changed.last_mut().unwrap() ^= 1;
    let mut journal_file = MemoryStorage::new(journal_literal(&changed, &expected, false));
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        decode(&before, Added::GeneratedText)
            .qualify_retained_recovery(checked, CatalogScope::new(3062), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(journal_file.writes, 0);
}

#[test]
fn original_v2_and_tcat5_cannot_enter_retained_recovery_even_for_equal_payload_bytes() {
    let before = sources().into_iter().next().unwrap();
    let expected = raw::append(&before, "More", Added::GeneratedNumber);
    let mut target = MemoryStorage::new(before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal_file,
            &before,
            &expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
    let checked = journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_add_table_recovery(
            checked,
            decode(&before, Added::GeneratedNumber),
            CatalogScope::new(3071),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);

    let mut journal_file = MemoryStorage::new(journal_literal(&before, &expected, false));
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let old = NativeAddTableIntent::new_mixed(
        definition(Added::GeneratedNumber),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        old.qualify_retained_recovery(checked, CatalogScope::new(3072), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(journal_file.writes, 0);
}
