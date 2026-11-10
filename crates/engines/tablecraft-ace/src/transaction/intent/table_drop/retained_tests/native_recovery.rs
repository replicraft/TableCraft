//! V3 recovery reconstructs the exact retained table operation under a fresh scope.
use super::{budget, literal};
use crate::{NativeImage, NativeTableDropIntent, qualify_native_table_drop_recovery};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_table_drop::retained::{self as raw, CASES, Case},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn wire(source: &raw::Source) -> Vec<u8> {
    literal(
        &source.bytes,
        source.definition,
        u8::try_from(source.catalog_root).unwrap(),
        "Table1",
    )
}

fn decode(source: &raw::Source) -> NativeTableDropIntent {
    NativeTableDropIntent::decode_retained(&wire(source), &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_table_drop_literal_reconstructs_original_and_appended_storage_in_fresh_scopes() {
    for case in CASES {
        let source = raw::source(case);
        let expected = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
        let plan = image(&source.bytes, 3901)
            .plan_recoverable_table_drop("Table1", &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
        assert!(plan.intent().is_retained());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&source)
        );
        assert_eq!(plan.before_bytes(), source.bytes);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.declared_rows() as usize, source.roots.len());
        let plan = plan
            .requalify_with_budgets(
                image(&source.bytes, 3902),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        for profile in [
            super::SourceProfile::Inline,
            super::SourceProfile::CreatorExternal,
            super::SourceProfile::Current,
        ] {
            let mut intent = decode(&source);
            intent.profile = profile;
            assert!(
                intent
                    .reconstruct(image(&source.bytes, 3903), &mut budget(), &|| false)
                    .is_err()
            );
        }
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        assert!(
            plan.prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(journal_file.writes, 0);
        assert!(journal_file.bytes.get_ref().is_empty());
    }
}

#[test]
fn retained_table_drop_pending_and_committed_v3_recover_and_confirm_same_checked_owner() {
    for case in CASES {
        let source = raw::source(case);
        let expected = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
        for committed in [false, true] {
            let plan = image(&source.bytes, 3911)
                .plan_recoverable_table_drop("Table1", &mut budget(), &|| false)
                .unwrap();
            let mut target = MemoryStorage::new(source.bytes.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&expected);
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = decode(&source)
                .qualify_retained_recovery(checked, CatalogScope::new(3912), &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(
                qualified.state(),
                if committed {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            let outcome = qualified
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &expected } else { &source.bytes }
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            decode(&source)
                .qualify_retained_recovery(checked, CatalogScope::new(3913), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

#[test]
fn retained_table_drop_recovery_rejects_forged_complements_and_v1_before_target_effects() {
    let source = raw::source(Case::SecondText);
    let expected = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        &source.roots,
    );
    for offset in [
        3 * 4096 + 3000,
        source.definition as usize * 4096 + 3000,
        (source.definition as usize + 1) * 4096 + 4000,
        10 * 4096 + 4000,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            decode(&source)
                .qualify_retained_recovery(checked, CatalogScope::new(3921), &mut budget(), &|| {
                    false
                })
                .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &source.bytes);
    }
    let mut target = MemoryStorage::new(source.bytes.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal_file,
            &source.bytes,
            &expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_table_drop_recovery(
            checked,
            decode(&source),
            CatalogScope::new(3922),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &source.bytes);
}
