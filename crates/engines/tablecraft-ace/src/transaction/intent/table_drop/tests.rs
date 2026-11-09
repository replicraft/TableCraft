//! Original wire bytes and actual checked-journal reconstruction over raw image pairs.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, V1State, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{canonical_creator_rename, canonical_table_drop as raw},
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(1251);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(before: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), scope, &mut budget(), &|| false).unwrap()
}

fn plan(pair: &raw::Pair) -> RecoverableTableDropPlan {
    image(&pair.before, SCOPE)
        .plan_recoverable_table_drop(&pair.table, &mut budget(), &|| false)
        .unwrap()
}

fn decode(bytes: &[u8]) -> NativeTableDropIntent {
    NativeTableDropIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

fn literal(pair: &raw::Pair, profile: u16) -> Vec<u8> {
    // Independent 80-byte header; these raw tables have native/root 7,
    // Tables parent 0x0f000001, and catalog page 4. No caller scope is persisted.
    let mut bytes = b"TCTD0001".to_vec();
    bytes.extend((80_u32 + pair.table.len() as u32).to_le_bytes());
    bytes.extend(profile.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend((pair.before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((pair.before.len() as u64).to_le_bytes());
    binding.update(&pair.before);
    bytes.extend(binding.finalize());
    bytes.extend(7_i32.to_le_bytes());
    bytes.extend(0x0f00_0001_i32.to_le_bytes());
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend(4_u32.to_le_bytes());
    bytes.push(pair.catalog_slot.try_into().unwrap());
    bytes.extend([0, 0, 0]);
    bytes.extend((pair.table.len() as u16).to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend(pair.table.as_bytes());
    bytes
}

#[test]
fn original_wire_and_full_raw_afters_requalify_under_fresh_catalog_scope() {
    let creator = canonical_creator_rename::originals().remove(0);
    for (pair, profile) in [
        (raw::unindexed(5, "Items", false), 1),
        (raw::unindexed(6, "Inventory", false), 1),
        (raw::creator(&creator), 2),
    ] {
        let reviewed = plan(&pair);
        let old_identity = reviewed.identity();
        let encoded = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(encoded, literal(&pair, profile));
        assert_eq!(reviewed.before_bytes(), pair.before);
        assert_eq!(reviewed.after_bytes(), pair.after);
        assert_eq!(reviewed.declared_rows(), 0);
        assert_eq!(
            decode(&encoded).encode(&mut budget(), &|| false).unwrap(),
            encoded
        );

        let fresh = reviewed
            .requalify_with_budgets(
                image(&pair.before, CatalogScope::new(1252)),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_ne!(fresh.identity().object_id(), old_identity.object_id());
        assert_eq!(fresh.identity().native_id(), old_identity.native_id());
        assert_eq!(fresh.intent().table(), pair.table);
        assert_eq!(fresh.after_bytes(), pair.after);
    }
}

#[test]
fn closed_decode_rejects_other_actions_versions_profiles_and_shapes_before_name_allocation() {
    let pair = raw::unindexed(5, "Items", false);
    let good = literal(&pair, 1);
    let mut cases = Vec::new();
    for offset in [0, 7, 8, 14, 73, 74, 75, 76, 78, 79, 80] {
        let mut bytes = good.clone();
        bytes[offset] ^= 0xff;
        cases.push(bytes);
    }
    let mut other_action = good.clone();
    other_action[..8].copy_from_slice(b"TCRN0001");
    cases.push(other_action);
    let mut bad_profile = good.clone();
    bad_profile[12..14].copy_from_slice(&3_u16.to_le_bytes());
    cases.push(bad_profile);
    for end in [0, 7, 79, good.len() - 1] {
        cases.push(good[..end].to_vec());
    }
    let mut trailing = good;
    trailing.push(0);
    cases.push(trailing);

    for bytes in cases {
        let mut work = budget();
        let remaining = work.remaining(Resource::Bytes);
        assert!(matches!(
            NativeTableDropIntent::decode(&bytes, &mut work, &|| false),
            Err(TableDropIntentError::InvalidRecord)
        ));
        assert_eq!(work.remaining(Resource::Bytes), remaining);
    }
}

#[test]
fn native_identity_source_profile_and_complete_before_binding_cannot_be_forged() {
    let pair = raw::unindexed(5, "Items", false);
    let encoded = literal(&pair, 1);
    let mut changed = pair.before.clone();
    changed[4096 + 3000] ^= 1;
    assert!(matches!(
        decode(&encoded).verify_before(&changed, &mut budget(), &|| false),
        Err(TableDropIntentError::BeforeMismatch)
    ));
    for component in 0..5 {
        let mut intent = decode(&encoded);
        match component {
            0 => intent.identity.native_id += 1,
            1 => intent.identity.parent_id += 1,
            2 => intent.identity.definition += 1,
            3 => intent.identity.catalog_page += 1,
            4 => intent.identity.catalog_slot += 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            intent.reconstruct(image(&pair.before, SCOPE), &mut budget(), &|| false),
            Err(TableDropIntentError::IdentityMismatch)
        ));
    }
    let mut wrong_profile = decode(&encoded);
    wrong_profile.profile = SourceProfile::CreatorExternal;
    assert!(matches!(
        wrong_profile.reconstruct(image(&pair.before, SCOPE), &mut budget(), &|| false),
        Err(TableDropIntentError::Drop(_))
    ));
}

#[test]
fn exact_checked_v1_recovers_pending_and_committed_drops_then_confirms_without_replay() {
    let pair = raw::unindexed(5, "Items", false);
    let encoded = literal(&pair, 1);
    for committed in [false, true] {
        let mut target = MemoryStorage::new(pair.before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&pair)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if committed {
            prepared.commit(&mut budget(), &|| false).unwrap();
        } else {
            drop(prepared);
            target.bytes.get_mut().copy_from_slice(&pair.after);
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_table_drop_recovery(
            checked,
            decode(&encoded),
            CatalogScope::new(1253),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected_state = if committed {
            V1State::Committed
        } else {
            V1State::Pending
        };
        assert_eq!(qualified.state(), expected_state);
        let outcome = qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        let expected_outcome = if committed {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        };
        assert_eq!(outcome, expected_outcome);
        let expected_bytes = if committed { &pair.after } else { &pair.before };
        assert_eq!(target.bytes.get_ref(), expected_bytes);

        let writes = target.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_table_drop_recovery(
            checked,
            decode(&encoded),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn checked_bytes_do_not_authorize_changed_after_or_an_unqualified_populated_source() {
    for populated in [false, true] {
        let pair = raw::unindexed(5, "Items", populated);
        let mut after = pair.after.clone();
        if !populated {
            after[7 * 4096 + 3000] ^= 1;
        }
        let mut target = MemoryStorage::new(pair.before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut journal,
                &pair.before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let result = qualify_native_table_drop_recovery(
            checked,
            decode(&literal(&pair, 1)),
            SCOPE,
            &mut budget(),
            &|| false,
        );
        if populated {
            assert!(matches!(result, Err(TableDropIntentError::Drop(_))));
        } else {
            assert!(matches!(result, Err(TableDropIntentError::Proof(_))));
        }
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &pair.before);
    }
}
