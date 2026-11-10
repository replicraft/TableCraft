//! Cold current DELETE reconstructs original roots, never trusts persisted AFTER.
use super::literal;
use crate::{
    NativeDeleteIntent, NativeImage, NativeRecoveryQualificationError, RowDelete,
    qualify_native_delete_recovery,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::{
        self as model,
        mixed::after_off::{
            current_delete as raw,
            deleted_field::populated::{CASES, Case, readdition::Target},
        },
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

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn original_record(before: &[u8], root: usize) -> Vec<u8> {
    // Independent actual DATA directory framing. A forwarded visible root names
    // its hidden terminal; physical slots are never derived from logical IDs.
    let mut page = 8;
    let mut slot = root;
    let mut visited = Vec::new();
    loop {
        assert!(!visited.contains(&(page, slot)));
        visited.push((page, slot));
        let base = page * 4096;
        assert!(slot < word(before, base + 12));
        let flags = word(before, base + 14 + slot * 2);
        let start = flags & 0x1fff;
        let end = if slot == 0 {
            4096
        } else {
            word(before, base + 14 + (slot - 1) * 2) & 0x1fff
        };
        assert!(start < end);
        if flags & 0x4000 == 0 {
            return before[base + start..base + end].to_vec();
        }
        let reference =
            u32::from_le_bytes(before[base + start..base + start + 4].try_into().unwrap());
        page = (reference >> 8) as usize;
        slot = (reference & 0xff) as usize;
    }
}

fn witnesses(source: &raw::Source, ordinals: &[usize]) -> Vec<RowDelete> {
    ordinals
        .iter()
        .map(|&ordinal| {
            RowDelete::new(
                ordinal,
                original_record(&source.bytes, source.roots[ordinal]),
            )
        })
        .collect()
}

fn wire(source: &raw::Source, ordinals: &[usize]) -> Vec<u8> {
    let payloads: Vec<_> = ordinals
        .iter()
        .map(|&ordinal| original_record(&source.bytes, source.roots[ordinal]))
        .collect();
    let frames: Vec<_> = ordinals
        .iter()
        .zip(&payloads)
        .map(|(&ordinal, payload)| (ordinal as u64, payload.as_slice()))
        .collect();
    literal(&source.bytes, "Table1", &frames)
}

fn selected(source: &raw::Source) -> Vec<usize> {
    if source.rows.len() == 1 {
        vec![0]
    } else {
        vec![source.rows.len() - 1, 0]
    }
}

#[test]
fn current_delete_literal_native_requests_requalify_under_fresh_scopes() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                let source = raw::source(case, target, filled, true, true);
                let ordinals = selected(&source);
                let roots: Vec<_> = ordinals.iter().map(|&row| source.roots[row]).collect();
                let expected = raw::after(&source.bytes, &roots);
                let plan = image(&source.bytes, 2831)
                    .plan_recoverable_current_row_deletes(
                        "Table1",
                        witnesses(&source, &ordinals),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                let encoded = wire(&source, &ordinals);
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    encoded
                );
                assert!(NativeDeleteIntent::decode(&encoded, &mut budget(), &|| false).is_err());
                let decoded =
                    NativeDeleteIntent::decode_current(&encoded, &mut budget(), &|| false).unwrap();
                assert_eq!(decoded.deletes()[0].row(), ordinals[0]);
                let fresh = plan
                    .requalify_with_budgets(
                        image(&source.bytes, 2832),
                        &mut budget(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(fresh.before_bytes(), source.bytes);
                assert_eq!(fresh.after_bytes(), expected);
                assert_eq!(fresh.deleted_rows(), ordinals.len());
            }
        }
    }
}

#[test]
fn current_delete_pending_and_committed_cold_recovery_reconstruct_original_roots() {
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            let source = raw::source(case, target, true, true, true);
            let ordinals = selected(&source);
            let roots: Vec<_> = ordinals.iter().map(|&row| source.roots[row]).collect();
            let after = raw::after(&source.bytes, &roots);
            let encoded = wire(&source, &ordinals);
            for committed in [false, true] {
                let plan = image(&source.bytes, 2831)
                    .plan_recoverable_current_row_deletes(
                        "Table1",
                        witnesses(&source, &ordinals),
                        &mut budget(),
                        &|| false,
                    )
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
                    target.bytes.get_mut().copy_from_slice(&after);
                }
                let checked =
                    journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeDeleteIntent::decode_current(&encoded, &mut budget(), &|| false).unwrap();
                qualify_native_delete_recovery(
                    checked,
                    intent,
                    CatalogScope::new(2833),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &after } else { &source.bytes }
                );
            }
        }
    }
}

#[test]
fn current_delete_cold_recovery_refuses_unselected_sources_and_forged_complements() {
    let before = model::source(model::Source::GuiRequiredZero);
    let encoded = literal(&before, "Table1", &[]);
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal_file,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
    let intent = NativeDeleteIntent::decode_current(&encoded, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_delete_recovery(
            checked,
            intent,
            CatalogScope::new(2831),
            &mut budget(),
            &|| false,
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);

    let source = raw::source(Case::MixedBinary, Target::ShortText, true, true, true);
    let after = raw::after(&source.bytes, &[source.roots[0]]);
    let encoded = wire(&source, &[0]);
    for offset in [
        7 * 4096 + 20,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        10 * 4096 + 4090,
    ] {
        let mut forged = after.to_vec();
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
        let intent =
            NativeDeleteIntent::decode_current(&encoded, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_delete_recovery(
                checked,
                intent,
                CatalogScope::new(2831),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &source.bytes);
    }
}
