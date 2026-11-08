//! Independent image and wire oracles for sealed native rename recovery.
use super::*;
use crate::{NativeImage, NativeRecoveryQualificationError, TableRenameError};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};
use tablecraft_core::limits::Resource;
use tablecraft_io::{
    IoError,
    journal::{self, DurableStorage, RecoveryOutcome, V1State, inspect_v1},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_catalog_names::{RenamePair, rename_pair};
use tablecraft_testkit::fixtures::access::{
    canonical_indexed_rename, canonical_ordinary_indexed::Case as IndexedCase,
};

const SCOPE: CatalogScope = CatalogScope::new(783);
const BYTES: u64 = 64 * 1024 * 1024;
const ROWS: u64 = 100_000;
const STEPS: u64 = 10_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, ROWS, STEPS)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(pair: &RenamePair) -> RecoverableNativeTableRenamePlan {
    image(&pair.before)
        .plan_recoverable_table_rename(&pair.old_name, &pair.new_name, &mut budget(), &|| false)
        .unwrap()
}

fn decoded(raw: &[u8]) -> NativeTableRenameIntent {
    NativeTableRenameIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

fn literal(pair: &RenamePair) -> Vec<u8> {
    literal_request(&pair.before, &pair.old_name, &pair.new_name, b"TCRN0001", 1)
}

fn literal_request(
    before: &[u8],
    old_name: &str,
    new_name: &str,
    magic: &[u8; 8],
    profile: u16,
) -> Vec<u8> {
    // Independent 84-byte TCRN header, original native identity and spellings.
    // No production wire offsets, encoder or captured intent are consulted.
    let mut raw = magic.to_vec();
    raw.extend_from_slice(&(84_u32 + old_name.len() as u32 + new_name.len() as u32).to_le_bytes());
    raw.extend_from_slice(&profile.to_le_bytes());
    raw.extend_from_slice(&[0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    raw.extend_from_slice(&7_i32.to_le_bytes());
    raw.extend_from_slice(&0x0f00_0001_i32.to_le_bytes());
    raw.extend_from_slice(&7_u32.to_le_bytes());
    raw.extend_from_slice(&4_u32.to_le_bytes());
    raw.extend_from_slice(&[8, 0, 0, 0]);
    raw.extend_from_slice(&(old_name.len() as u16).to_le_bytes());
    raw.extend_from_slice(&(new_name.len() as u16).to_le_bytes());
    raw.extend_from_slice(&0_u32.to_le_bytes());
    raw.extend_from_slice(old_name.as_bytes());
    raw.extend_from_slice(new_name.as_bytes());
    raw
}

#[test]
fn independent_wire_and_complete_images_survive_fresh_requalification() {
    for header in [5, 6] {
        for (old, new) in [
            ("Items", "Renamed"),
            ("Items", "I"),
            ("Items", "items"),
            ("Items", "Items"),
        ] {
            for populated in [false, true] {
                let pair = rename_pair(header, old, new, populated);
                let reviewed = plan(&pair);
                let raw = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
                assert_eq!(raw, literal(&pair));
                assert_eq!(decoded(&raw).encode(&mut budget(), &|| false).unwrap(), raw);
                let fresh = reviewed
                    .requalify_with_budgets(
                        image(&pair.before),
                        &mut budget(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert!(
                    fresh.before_bytes() == pair.before,
                    "complete original changed"
                );
                assert!(
                    fresh.after_bytes() == pair.after,
                    "independent complete AFTER differs"
                );
                assert_eq!(fresh.identity().native_id(), 7);
                assert_eq!(fresh.identity().catalog_slot(), 8);
                assert_eq!(fresh.changed(), old != new);
                assert_eq!(
                    fresh.intent().encode(&mut budget(), &|| false).unwrap(),
                    raw
                );
            }
        }
    }
}

#[test]
fn malformed_wire_refuses_before_name_allocation_and_does_not_change_action_family() {
    let pair = rename_pair(5, "Items", "Renamed", false);
    let raw = literal(&pair);
    let mut cases = Vec::new();
    for offset in [8, 14, 73, 74, 75, 76, 78, 80, 84] {
        let mut changed = raw.clone();
        changed[offset] ^= 0xff;
        cases.push(changed);
    }
    let mut control_name = raw.clone();
    control_name[84] = 0;
    cases.push(control_name);
    let mut appended = raw.clone();
    appended.push(0);
    cases.push(appended);
    for end in 0..raw.len() {
        cases.push(raw[..end].to_vec());
    }
    for altered in cases {
        let mut limits = ResourceBudget::new(0, 0, STEPS);
        let error = NativeTableRenameIntent::decode(&altered, &mut limits, &|| false)
            .err()
            .expect("malformed wire must fail");
        assert!(!matches!(
            error,
            TableRenameIntentError::Inspect(InspectError::Limit(_))
        ));
        assert_eq!(limits.remaining(Resource::Bytes), 0);
    }
    let mut future = raw.clone();
    future[7] = b'3';
    assert!(matches!(
        NativeTableRenameIntent::decode(&future, &mut budget(), &|| false),
        Err(TableRenameIntentError::UnsupportedVersion)
    ));
    let mut profile = raw.clone();
    profile[12] = 2;
    assert!(matches!(
        NativeTableRenameIntent::decode(&profile, &mut budget(), &|| false),
        Err(TableRenameIntentError::UnsupportedProfile)
    ));
    let mut other_action = raw;
    other_action[..8].copy_from_slice(b"TCNI0001");
    assert!(matches!(
        NativeTableRenameIntent::decode(&other_action, &mut budget(), &|| false),
        Err(TableRenameIntentError::InvalidRecord)
    ));
}

#[test]
fn stale_original_and_tampered_identity_or_proposal_publish_no_requalified_plan() {
    let pair = rename_pair(5, "Items", "Renamed", true);
    let mut stale = pair.before.clone();
    let last = stale.len() - 1;
    stale[last] ^= 1;
    assert!(matches!(
        plan(&pair).requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    for identity in [false, true] {
        let mut reviewed = plan(&pair);
        if identity {
            reviewed.intent.identity.native_id += 1;
        } else {
            reviewed.intent.new_name = SchemaName::new("Different".to_owned()).unwrap();
        }
        assert!(
            reviewed
                .requalify_with_budgets(image(&pair.before), &mut budget(), &mut budget(), &|| {
                    false
                })
                .is_err()
        );
    }
}

#[test]
fn proof_and_native_pools_are_independent_and_terminal_cancellation_refuses() {
    let pair = rename_pair(5, "Items", "Renamed", false);
    for native_empty in [false, true] {
        let mut native = if native_empty {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if native_empty {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            plan(&pair)
                .requalify_with_budgets(image(&pair.before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    let polls = Cell::new(0);
    plan(&pair)
        .requalify_with_budgets(image(&pair.before), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    for stop in [1, polls.get() / 2, polls.get()] {
        let seen = Cell::new(0);
        assert!(matches!(
            plan(&pair).requalify_with_budgets(
                image(&pair.before),
                &mut budget(),
                &mut budget(),
                &|| {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                }
            ),
            Err(NativeRecoveryQualificationError::Inspect(
                InspectError::Cancelled
            )) | Err(NativeRecoveryQualificationError::Rename(
                TableRenameError::Inspect(InspectError::Cancelled)
            )) | Err(NativeRecoveryQualificationError::TableRenameIntent(
                TableRenameIntentError::Inspect(InspectError::Cancelled)
            )) | Err(NativeRecoveryQualificationError::TableRenameIntent(
                TableRenameIntentError::Binding(NativeIntentError::Inspect(
                    InspectError::Cancelled
                ))
            ))
        ));
    }
}

// Local storage glue observes exactly this schema journal boundary. Complete raw
// image construction remains in the shared independent fixture owner above.
struct Storage {
    bytes: Cursor<Vec<u8>>,
    writes: usize,
}

impl Storage {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            writes: 0,
        }
    }
}

impl Read for Storage {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for Storage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for Storage {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.write(input)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush must not replace the test storage barrier")
    }
}

impl DurableStorage for Storage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}

#[test]
fn pending_committed_and_torn_targets_recover_through_same_qualified_v1_session() {
    let pair = rename_pair(5, "Items", "Renamed", true);
    let raw = literal(&pair);
    assert_recovery(&pair.before, &pair.after, &raw, || plan(&pair));
}

fn assert_recovery(
    before: &[u8],
    after: &[u8],
    raw: &[u8],
    build: impl Fn() -> RecoverableNativeTableRenamePlan,
) {
    for state in 0..4 {
        let mut target = Storage::new(before.to_vec());
        let mut journal = Storage::new(Vec::new());
        let prepared = build()
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if matches!(state, 1 | 3) {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                // Independent V1 golden: retain half of the 40-byte commit marker.
                let partial_length = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(partial_length);
            }
        } else {
            drop(prepared);
            if state == 2 {
                let middle = after.len() / 2;
                target.bytes.get_mut()[..middle].copy_from_slice(&after[..middle]);
            }
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_table_rename_recovery(
            checked,
            decoded(raw),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified.state(),
            if state == 1 {
                V1State::Committed
            } else {
                V1State::Pending
            }
        );
        let outcome = qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if state == 1 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        let expected = if state == 1 { after } else { before };
        assert_eq!(target.bytes.get_ref().as_slice(), expected);
        let writes = target.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_table_rename_recovery(checked, decoded(raw), SCOPE, &mut budget(), &|| {
            false
        })
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn journal_replacement_tamper_and_stale_prepare_do_not_gain_schema_authority() {
    let pair = rename_pair(5, "Items", "Renamed", false);
    let raw = literal(&pair);
    let mut after = pair.after.clone();
    let last = after.len() - 1;
    after[last] ^= 1;
    let mut target = Storage::new(pair.before.clone());
    let mut journal = Storage::new(Vec::new());
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
    let writes = target.writes;
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_table_rename_recovery(checked, decoded(&raw), SCOPE, &mut budget(), &|| {
            false
        }),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, writes);
    let mut stale = pair.before.clone();
    stale[last] ^= 1;
    let mut target = Storage::new(stale);
    let mut journal = Storage::new(Vec::new());
    assert!(
        plan(&pair)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
}

#[test]
fn decoding_exact_resource_limits_and_last_cancellation_are_enforced() {
    let raw = literal(&rename_pair(5, "Items", "Renamed", false));
    let mut measured = budget();
    let polls = Cell::new(0);
    NativeTableRenameIntent::decode(&raw, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();

    for (resource, available) in [(Resource::Bytes, BYTES), (Resource::Steps, STEPS)] {
        let needed = available - measured.remaining(resource);
        assert!(needed > 0);
        for (quota, admitted) in [(needed, true), (needed - 1, false)] {
            let mut limits = match resource {
                Resource::Bytes => ResourceBudget::new(quota, ROWS, STEPS),
                Resource::Steps => ResourceBudget::new(BYTES, ROWS, quota),
                Resource::Rows => unreachable!(),
            };
            let result = NativeTableRenameIntent::decode(&raw, &mut limits, &|| false);
            assert_eq!(result.is_ok(), admitted);
        }
    }

    let seen = Cell::new(0);
    assert!(matches!(
        NativeTableRenameIntent::decode(&raw, &mut budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() == polls.get()
        }),
        Err(TableRenameIntentError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn indexed_v2_literal_wire_and_complete_images_survive_fresh_requalification() {
    for header in [5, 6] {
        for case in [
            IndexedCase::CounterPrimary,
            IndexedCase::NullableOrdinary,
            IndexedCase::AliasHistory,
            IndexedCase::RetainedTree,
            IndexedCase::UnknownIndexHeader,
        ] {
            let pair = canonical_indexed_rename::pair(header, case, "Items", "雪🙂");
            let reviewed = image(&pair.before)
                .plan_recoverable_indexed_table_rename("Items", "雪🙂", &mut budget(), &|| false)
                .unwrap();
            let raw = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(
                raw,
                literal_request(&pair.before, "Items", "雪🙂", b"TCRN0002", 2)
            );
            assert_eq!(decoded(&raw).encode(&mut budget(), &|| false).unwrap(), raw);
            let fresh = reviewed
                .requalify_with_budgets(image(&pair.before), &mut budget(), &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(fresh.before_bytes(), pair.before);
            assert_eq!(fresh.after_bytes(), pair.after);
            assert_eq!(
                fresh.intent().encode(&mut budget(), &|| false).unwrap(),
                raw
            );
        }
    }
}

#[test]
fn intent_binds_actual_planner_purpose_even_when_complete_bytes_are_identical() {
    let pair = rename_pair(5, "Items", "Renamed", true);
    let legacy = image(&pair.before)
        .plan_table_rename("Items", "Renamed", &mut budget(), &|| false)
        .unwrap();
    let explicit = image(&pair.before)
        .plan_indexed_table_rename("Items", "Renamed", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(legacy.before_bytes(), explicit.before_bytes());
    assert_eq!(legacy.after_bytes(), explicit.after_bytes());
    let legacy_intent =
        NativeTableRenameIntent::capture(&legacy, &mut budget(), &|| false).unwrap();
    let indexed_intent =
        NativeTableRenameIntent::capture(&explicit, &mut budget(), &|| false).unwrap();
    assert!(legacy_intent.verify_plan(&legacy).is_ok());
    assert!(indexed_intent.verify_plan(&explicit).is_ok());
    assert!(matches!(
        legacy_intent.verify_plan(&explicit),
        Err(TableRenameIntentError::IdentityMismatch)
    ));
    assert!(matches!(
        indexed_intent.verify_plan(&legacy),
        Err(TableRenameIntentError::IdentityMismatch)
    ));
    assert_eq!(
        legacy_intent.encode(&mut budget(), &|| false).unwrap(),
        literal(&pair)
    );
    assert_eq!(
        indexed_intent.encode(&mut budget(), &|| false).unwrap(),
        literal_request(&pair.before, "Items", "Renamed", b"TCRN0002", 2)
    );
}

#[test]
fn version_profile_pairs_are_closed_and_v2_framing_refuses_before_allocating_names() {
    let pair = canonical_indexed_rename::pair(5, IndexedCase::AliasHistory, "Items", "Renamed");
    for (magic, profile) in [(b"TCRN0001", 2), (b"TCRN0002", 1), (b"TCRN0002", 3)] {
        let raw = literal_request(&pair.before, "Items", "Renamed", magic, profile);
        let mut limits = ResourceBudget::new(0, 0, STEPS);
        assert!(matches!(
            NativeTableRenameIntent::decode(&raw, &mut limits, &|| false),
            Err(TableRenameIntentError::UnsupportedProfile)
        ));
        assert_eq!(limits.remaining(Resource::Bytes), 0);
    }
    let raw = literal_request(&pair.before, "Items", "Renamed", b"TCRN0002", 2);
    for offset in [8, 14, 73, 74, 75, 76, 78, 80, 84] {
        let mut malformed = raw.clone();
        malformed[offset] ^= 0xff;
        let mut limits = ResourceBudget::new(0, 0, STEPS);
        assert!(matches!(
            NativeTableRenameIntent::decode(&malformed, &mut limits, &|| false),
            Err(TableRenameIntentError::InvalidRecord)
        ));
        assert_eq!(limits.remaining(Resource::Bytes), 0);
    }
    let mut unknown = raw;
    unknown[..8].copy_from_slice(b"TCRN0003");
    assert!(matches!(
        NativeTableRenameIntent::decode(&unknown, &mut budget(), &|| false),
        Err(TableRenameIntentError::UnsupportedVersion)
    ));
}

#[test]
fn indexed_v2_recovers_pending_committed_and_torn_journals_without_legacy_fallback() {
    for header in [5, 6] {
        for case in [IndexedCase::AliasHistory, IndexedCase::RetainedTree] {
            let pair = canonical_indexed_rename::pair(header, case, "Items", "Renamed");
            let raw = literal_request(&pair.before, "Items", "Renamed", b"TCRN0002", 2);
            let build = || {
                image(&pair.before)
                    .plan_recoverable_indexed_table_rename(
                        "Items",
                        "Renamed",
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            };
            assert_recovery(&pair.before, &pair.after, &raw, build);

            let mut target = Storage::new(pair.before.clone());
            let mut journal = Storage::new(Vec::new());
            drop(
                build()
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap(),
            );
            let legacy = literal_request(&pair.before, "Items", "Renamed", b"TCRN0001", 1);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_table_rename_recovery(
                    checked,
                    decoded(&legacy),
                    SCOPE,
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::Rename(
                    TableRenameError::Source
                ))
            ));
            assert_eq!(target.writes, 0);
        }
    }
}

#[test]
fn indexed_v2_stale_original_and_tampered_journal_after_publish_no_authority() {
    let pair = canonical_indexed_rename::pair(5, IndexedCase::CounterPrimary, "Items", "Renamed");
    let build = || {
        image(&pair.before)
            .plan_recoverable_indexed_table_rename("Items", "Renamed", &mut budget(), &|| false)
            .unwrap()
    };
    let mut stale = pair.before.clone();
    let last = stale.len() - 1;
    stale[last] ^= 1;
    assert!(matches!(
        build().requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));

    let raw = literal_request(&pair.before, "Items", "Renamed", b"TCRN0002", 2);
    let mut after = pair.after.clone();
    after[last] ^= 1;
    let mut target = Storage::new(pair.before.clone());
    let mut journal = Storage::new(Vec::new());
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
    assert!(matches!(
        qualify_native_table_rename_recovery(checked, decoded(&raw), SCOPE, &mut budget(), &|| {
            false
        }),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
}
