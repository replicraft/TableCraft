//! Exact typed cleanup regenerates whole images under the same checked V3 owner.
use super::{Error, Facts, Intent, budget, literal};
use crate::{
    NativeImage, NativeRecoveryQualificationError, RecoverableFieldDropPlan,
    qualify_native_field_drop_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{
        exact_values, field_changes, fields, fixed_scalars,
    },
    support::io::journal::MemoryStorage,
};

struct Source {
    bytes: Vec<u8>,
    table: &'static str,
    column: &'static str,
    number: u16,
    definition: u32,
    root: usize,
}

fn source(column: &'static str) -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let (bytes, table, number) = if column == "DateValue" {
        let empty = fixed_scalars::append_table(&seed, "Measurements");
        (fixed_scalars::append(&empty, definition), "Measurements", 7)
    } else {
        let empty = exact_values::append_table(&seed, "ExactValues");
        (
            exact_values::append(
                &empty,
                definition,
                Some([0x42; 16]),
                Some(90_071_992_547_409_931_234),
            ),
            "ExactValues",
            3,
        )
    };
    Source {
        bytes,
        table,
        column,
        number,
        definition,
        root,
    }
}

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(source: &Source) -> RecoverableFieldDropPlan {
    image(&source.bytes, 1801)
        .plan_recoverable_retained_typed_tail_field_drop(
            source.table,
            source.column,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn typed_fixed_and_variable_tails_requalify_pending_committed_and_settled_v3_images() {
    for column in ["DateValue", "Amount"] {
        let source = source(column);
        let after = field_changes::drop_tail(&source.bytes, source.definition, source.root, column);
        let wire = literal(
            &source.bytes,
            Facts {
                definition: source.definition,
                number: source.number,
                column_id: source.number,
            },
            source.table,
            column,
        );
        for committed in [false, true] {
            let reviewed = plan(&source);
            assert!(reviewed.intent().is_retained_typed());
            assert!(!reviewed.intent().is_retained());
            assert_eq!(
                reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
                wire
            );
            assert_image(reviewed.after_bytes(), &after);
            let fresh = reviewed
                .requalify_with_budgets(
                    image(&source.bytes, 1802),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut target = MemoryStorage::new(source.bytes.clone());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = fresh
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            Intent::decode_retained_typed(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1803), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_image(
                target.bytes.get_ref(),
                if committed { &after } else { &source.bytes },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            Intent::decode_retained_typed(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1804), &mut budget(), &|| {
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
fn typed_drop_refuses_original_v1_and_forged_after_before_target_effects() {
    let source = source("Amount");
    let reviewed = plan(&source);
    let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
    let mut target = MemoryStorage::new(source.bytes.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        reviewed.prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    let mut after =
        field_changes::drop_tail(&source.bytes, source.definition, source.root, source.column);
    after[7 * 4096 + 100] ^= 1;
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &source.bytes,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        Intent::decode_retained_typed(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1805), &mut budget(), &|| false),
        Err(Error::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    let mut original_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut original_journal,
            &source.bytes,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut original_journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_field_drop_recovery(
            checked,
            Intent::decode_retained_typed(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(1806),
            &mut budget(),
            &|| false
        ),
        Err(Error::InvalidRecord)
    ));
    assert_eq!(target.writes, 0);
    assert_image(target.bytes.get_ref(), &source.bytes);
}
