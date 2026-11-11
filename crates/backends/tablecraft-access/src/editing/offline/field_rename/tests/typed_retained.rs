//! Typed rename journals retain the real offline owner across cold recovery.
use crate::editing::offline::{
    CompletionKind, OfflineWriteSession, SchemaChange, VerifiedImage,
    test_support::{
        assert_completion,
        saved_field::{FieldPlan, retain_retained_pair, retired_namespace},
    },
};
use std::{fs, io::Cursor, path::Path};
use tablecraft_ace::{NativeImage, RecoverableFieldRenamePlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    exact_values as raw, field_changes, fields,
};

const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const ROW_COUNT_OFFSET: usize = 12;
const SCOPE: CatalogScope = CatalogScope::new(1611);
const TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn open(path: &Path) -> OfflineWriteSession {
    OfflineWriteSession::open(path, SCOPE, &mut budget(), &|| false).unwrap()
}

fn review(before: &[u8], column: &str) -> RecoverableFieldRenamePlan {
    NativeImage::capture_retained(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false)
        .unwrap()
        .plan_recoverable_retained_typed_field_rename(
            "ExactValues",
            column,
            "ExpandedField",
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing image byte",
    );
}

#[test]
fn typed_retained_rename_cold_and_settled_recovery_publish_only_bound_schema_effects() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE_BYTES) as u32;
    // The independent fixture appends one catalog root at the original frontier.
    let count_at = CATALOG_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET;
    let catalog_root = usize::from(u16::from_le_bytes(
        seed[count_at..count_at + size_of::<u16>()]
            .try_into()
            .unwrap(),
    ));
    let empty = raw::append_table(&seed, "ExactValues");
    let before = raw::append(
        &empty,
        definition,
        Some(TOKEN),
        Some(90_071_992_547_409_931_234),
    );

    for column in ["Token", "Amount"] {
        let after =
            field_changes::rename(&before, definition, catalog_root, column, "ExpandedField");
        for committed in [false, true] {
            for retired in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("typed-rename.accdb");
                let bundle = directory.path().join("journal");
                fs::write(&path, &before).unwrap();
                let plan = review(&before, column);
                assert!(plan.intent().is_retained_typed());
                assert_image(plan.after_bytes(), &after);
                let owner = retain_retained_pair(
                    open(&path),
                    &bundle,
                    FieldPlan::Rename(plan),
                    committed,
                    retired,
                    &mut budget(),
                );
                owner.release().unwrap();
                let (owner, receipt) = if retired {
                    open(&path)
                        .confirm_retired(
                            &retired_namespace(directory.path()),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    open(&path)
                        .recover_update(&bundle, &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (receipt.matched_rows(), receipt.changed_rows()),
                    (None, None)
                );
                assert_eq!(
                    receipt.schema_change(),
                    committed.then_some(SchemaChange::ColumnRenamed)
                );
                assert_eq!(receipt.query_change(), None);
                assert_eq!(receipt.database_options_change(), None);
                assert_completion(&receipt);
                if retired {
                    assert_eq!(receipt.kind(), CompletionKind::Confirmed);
                    assert_eq!(
                        receipt.verified_image(),
                        Some(if committed {
                            VerifiedImage::Replacement
                        } else {
                            VerifiedImage::Original
                        }),
                    );
                } else {
                    assert_eq!(receipt.kind(), CompletionKind::Recovered);
                    assert_eq!(
                        receipt.outcome(),
                        Some(if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        }),
                    );
                }
                owner.release().unwrap();
                assert_image(
                    &fs::read(&path).unwrap(),
                    if committed { &after } else { &before },
                );
            }
        }
    }
}
