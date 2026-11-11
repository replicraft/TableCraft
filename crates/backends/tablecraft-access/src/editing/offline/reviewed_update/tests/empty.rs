//! A real empty SQL selection retains the named target and original V3 action.
use super::{budget, open};
use crate::editing::{
    offline::{CompletionKind, test_support::retained_update::assert_recovery},
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::joined_update;

#[test]
fn retained_empty_long_sql_uses_explicit_original_profile_and_saves_zero_counts() {
    let before = joined_update::source().bytes;
    let image = NativeImage::capture_retained(
        &mut Cursor::new(&before),
        CatalogScope::new(1724),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let query = parse_query(
        "UPDATE Table1 SET Score=25 WHERE ID<0",
        ParseLimits::default(),
    )
    .unwrap();
    let reviewed =
        sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
            .unwrap();
    assert_eq!(reviewed.matched_rows(), 0);
    assert_eq!(reviewed.backend().changed_rows(), 0);
    assert_eq!(
        reviewed.backend().intent().profile(),
        NativeUpdateProfile::Ace16RetainedEmptyRows
    );
    assert!(reviewed.backend().after_bytes() == before);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("empty.accdb");
    fs::write(&path, &before).unwrap();
    let bundle = directory.path().join("journal");
    let staged = open(&path)
        .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert!(!bundle.exists());
    assert!(fs::read(&path).unwrap() == before);
    let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
    assert_eq!(receipt.kind(), CompletionKind::Applied);
    assert_eq!(
        (receipt.matched_rows(), receipt.changed_rows()),
        (Some(0), Some(0))
    );
    assert!(receipt.schema_change().is_none());
    owner.release().unwrap();
    assert!(fs::read(&path).unwrap() == before);
    assert!(!bundle.exists());
}

#[test]
fn empty_marker_recovery_distinguishes_journal_outcomes_despite_equal_images() {
    let before = joined_update::source().bytes;
    assert_recovery(
        &before,
        &before,
        NativeUpdateProfile::Ace16RetainedEmptyRows,
        b"TCNI0024",
        || {
            let image = NativeImage::capture_retained(
                &mut Cursor::new(&before),
                CatalogScope::new(1724),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let query = parse_query(
                "UPDATE Table1 SET Score=25 WHERE ID<0",
                ParseLimits::default(),
            )
            .unwrap();
            sql::plan_recoverable_batch_update(image, query.query(), &mut budget(), None, &|| false)
                .unwrap()
                .into_backend()
        },
    );
}
