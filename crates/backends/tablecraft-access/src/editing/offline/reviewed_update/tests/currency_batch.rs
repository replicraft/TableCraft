//! Real Currency SQL retains distinct original selection and V3 offline recovery.
use super::{SCOPE, budget, open};
use crate::editing::{
    offline::{CompletionKind, test_support::retained_update::assert_recovery},
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::currency::batch;

#[test]
fn currency_count_selects_its_exact_original_then_applies_one_complete_image() {
    for (sql, values, count, profile) in [
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(Nz(KeyValue,0)+1.25)",
            [Some(24845), Some(12500)],
            2,
            NativeUpdateProfile::Ace16RetainedCurrencyColumnBatch,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(5.25) WHERE KeyValue>CCur(100)",
            [Some(12345), None],
            0,
            NativeUpdateProfile::Ace16RetainedCurrencyColumnBatch,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(KeyValue+1.25) WHERE KeyValue IS NOT NULL",
            [Some(24845), None],
            1,
            NativeUpdateProfile::Ace16RetainedCurrencyRows,
        ),
    ] {
        let source = batch::values(&[Some(12345), None], &values);
        let image = NativeImage::capture_retained(
            &mut Cursor::new(&source.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let reviewed =
            sql::plan_recoverable_batch_update(image, parsed.query(), &mut budget(), None, &|| {
                false
            })
            .unwrap();
        assert_eq!(reviewed.backend().intent().profile(), profile);
        assert_eq!(reviewed.matched_rows(), count);
        assert_eq!(reviewed.backend().changed_rows(), count);
        assert_eq!(reviewed.backend().after_bytes(), source.expected.as_slice());

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("currency.accdb");
        fs::write(&path, &source.before).unwrap();
        let bundle = directory.path().join("journal");
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert!(!bundle.exists());
        assert_eq!(fs::read(&path).unwrap(), source.before);
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.kind(), CompletionKind::Applied);
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(count), Some(count))
        );
        assert!(receipt.schema_change().is_none());
        owner.release().unwrap();
        assert_eq!(fs::read(path).unwrap(), source.expected);
        assert!(!bundle.exists());
    }
}

#[test]
fn currency_original_dispatch_recovers_changed_and_identical_images_from_real_v3_states() {
    for (sql, values) in [
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(Nz(KeyValue,0)+1.25)",
            [Some(24845), Some(12500)],
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(5.25) WHERE KeyValue>CCur(100)",
            [Some(12345), None],
        ),
    ] {
        let source = batch::values(&[Some(12345), None], &values);
        assert_recovery(
            &source.before,
            &source.expected,
            NativeUpdateProfile::Ace16RetainedCurrencyColumnBatch,
            b"TCNI0027",
            || {
                let image = NativeImage::capture_retained(
                    &mut Cursor::new(&source.before),
                    SCOPE,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let parsed = parse_query(sql, ParseLimits::default()).unwrap();
                sql::plan_recoverable_batch_update(
                    image,
                    parsed.query(),
                    &mut budget(),
                    None,
                    &|| false,
                )
                .unwrap()
                .into_backend()
            },
        );
    }
}
