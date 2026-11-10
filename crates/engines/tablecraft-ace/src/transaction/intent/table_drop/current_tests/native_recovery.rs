//! Current table DROP cold recovery rederives rows, metadata and the full image.
use super::{budget, literal};
use crate::{NativeImage, NativeTableDropIntent, qualify_native_table_drop_recovery};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::mixed::after_off::deleted_field::populated::{
            Case, readdition::Target,
        },
        canonical_table_drop::{self as original, current as raw},
    },
    support::io::journal::MemoryStorage,
};

fn sources() -> [raw::Source; 3] {
    [
        raw::source(
            Case::NumberAbsent,
            Target::Number,
            false,
            raw::History::Initial,
        ),
        raw::source(
            Case::MixedBinary,
            Target::ShortText,
            true,
            raw::History::Inserted,
        ),
        raw::source(Case::IdOnlyText, Target::Number, false, raw::History::Empty),
    ]
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

fn decode(before: &[u8]) -> NativeTableDropIntent {
    NativeTableDropIntent::decode_current(&literal(before), &mut budget(), &|| false).unwrap()
}

#[test]
fn current_table_drop_literal_requests_requalify_complete_images_in_fresh_scopes() {
    for source in sources() {
        let expected = raw::after(&source);
        let plan = image(&source.bytes, 2911)
            .plan_recoverable_table_drop("Table1", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), source.bytes);
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.declared_rows() as usize, source.roots.len());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&source.bytes)
        );

        let plan = plan
            .requalify_with_budgets(
                image(&source.bytes, 2912),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);

        for profile in [
            super::SourceProfile::Inline,
            super::SourceProfile::CreatorExternal,
        ] {
            let mut wrong_family = decode(&source.bytes);
            wrong_family.profile = profile;
            assert!(
                wrong_family
                    .reconstruct(image(&source.bytes, 2913), &mut budget(), &|| false)
                    .is_err()
            );
        }
    }
}

#[test]
fn current_table_drop_pending_and_committed_recovery_retains_checked_journal_ownership() {
    for source in sources() {
        let expected = raw::after(&source);
        for committed in [false, true] {
            let plan = image(&source.bytes, 2921)
                .plan_recoverable_table_drop("Table1", &mut budget(), &|| false)
                .unwrap();
            let mut target = MemoryStorage::new(source.bytes.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&expected);
            }
            let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_table_drop_recovery(
                checked,
                decode(&source.bytes),
                CatalogScope::new(2922),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified.state(),
                if committed {
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
            let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
            qualify_native_table_drop_recovery(
                checked,
                decode(&source.bytes),
                CatalogScope::new(2923),
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

#[test]
fn current_table_drop_recovery_refuses_forged_complements_and_other_sources_before_effects() {
    let source = raw::source(
        Case::MixedBinary,
        Target::ShortText,
        true,
        raw::History::Inserted,
    );
    let expected = raw::after(&source);
    for offset in [
        3 * 4096 + 3000,
        7 * 4096 + 3000,
        8 * 4096 + 4000,
        10 * 4096 + 4000,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(source.bytes.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_native_table_drop_recovery(
                checked,
                decode(&source.bytes),
                CatalogScope::new(2931),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &source.bytes);
    }

    let unsupported = original::unindexed(5, "Table1", true);
    assert!(
        decode(&unsupported.before)
            .reconstruct(image(&unsupported.before, 2932), &mut budget(), &|| false)
            .is_err()
    );
}
