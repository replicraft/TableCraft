//! Actual native declarations select fixed batch descriptors and retain V3 authority.
use super::{SCOPE, budget, open};
use crate::editing::{
    offline::test_support::{assert_completion, retained_update::assert_recovery},
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{
    NativeFittedEditIntent, NativeFixedScalarKind, NativeImage, NativeUpdateProfile,
};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields,
    fixed_scalars::{self as raw, Column, Scalar},
};

fn cases() -> [(Column, NativeFixedScalarKind, &'static str, Scalar); 6] {
    [
        (
            Column::ByteValue,
            NativeFixedScalarKind::Byte,
            "255",
            Scalar::Byte(255),
        ),
        (
            Column::ShortValue,
            NativeFixedScalarKind::Int16,
            "-32768",
            Scalar::Int16(-32768),
        ),
        (
            Column::LargeValue,
            NativeFixedScalarKind::Int64,
            "9007199254740993",
            Scalar::Int64(9007199254740993),
        ),
        (
            Column::SingleValue,
            NativeFixedScalarKind::Single,
            "CSng(1.5)",
            Scalar::SingleBits(0x3fc0_0000),
        ),
        (
            Column::DoubleValue,
            NativeFixedScalarKind::Double,
            "CDbl(2.25)",
            Scalar::DoubleBits(0x4002_0000_0000_0000),
        ),
        (
            Column::DateValue,
            NativeFixedScalarKind::DateTime,
            "#1900-01-01#",
            Scalar::DateTimeBits(0x4000_0000_0000_0000),
        ),
    ]
}

fn images(column: Column, value: Scalar, selected: &[usize]) -> (Vec<u8>, Vec<u8>) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "Measurements");
    let first = raw::append(&empty, definition);
    let before = raw::append(&first, definition);
    // NO_CLONE: Preserve the independent immutable BEFORE while raw field bytes
    // form the complete expected AFTER for this selected set of original rows.
    let mut after = before.to_vec();
    for &row in selected {
        after = raw::update(&after, definition, row, column, Some(value));
    }
    (before, after)
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
fn actual_six_native_kinds_select_batch_zero_and_singleton_originals_before_apply() {
    for (column, kind, expression, value) in cases() {
        for (criterion, selected, profile) in [
            (
                "",
                &[0_usize, 1][..],
                NativeUpdateProfile::Ace16RetainedFixedScalarColumnBatch,
            ),
            (
                " WHERE ID<0",
                &[][..],
                NativeUpdateProfile::Ace16RetainedFixedScalarColumnBatch,
            ),
            (
                " WHERE ID=1",
                &[0_usize][..],
                NativeUpdateProfile::Ace16RetainedFixedScalarRows,
            ),
        ] {
            let (before, after) = images(column, value, selected);
            let image = NativeImage::capture_retained(
                &mut Cursor::new(&before),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let statement = format!(
                "UPDATE Measurements SET [{}]={expression}{criterion}",
                column.name()
            );
            let parsed = parse_query(&statement, ParseLimits::default()).unwrap();
            let review = sql::plan_recoverable_batch_update(
                image,
                parsed.query(),
                &mut budget(),
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(review.backend().intent().profile(), profile);
            assert_eq!(
                (review.matched_rows(), review.backend().changed_rows()),
                (selected.len(), selected.len())
            );
            assert_image(review.backend().after_bytes(), &after);
            if profile == NativeUpdateProfile::Ace16RetainedFixedScalarColumnBatch {
                let wire = review
                    .backend()
                    .intent()
                    .encode(&mut budget(), &|| false)
                    .unwrap();
                let original = NativeFittedEditIntent::decode_retained_fixed_scalar_column_batch(
                    &wire,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(original.groups().len(), 1);
                assert_eq!(original.groups()[0].fixed_scalar_kind(), Some(kind));
            }

            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("measurements.accdb");
            let bundle = directory.path().join("journal");
            fs::write(&path, &before).unwrap();
            let staged = open(&path)
                .stage_reviewed_update_with_budgets(review, &mut budget(), &mut budget(), &|| false)
                .unwrap();
            assert!(!bundle.exists());
            let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(
                (receipt.matched_rows(), receipt.changed_rows()),
                (Some(selected.len()), Some(selected.len()))
            );
            assert!(receipt.schema_change().is_none());
            assert_completion(&receipt);
            owner.release().unwrap();
            assert_image(&fs::read(path).unwrap(), &after);
            assert!(!bundle.exists());
        }
    }
}

#[test]
fn six_kind_originals_recover_changed_and_zero_images_without_losing_the_descriptor() {
    for (column, _kind, expression, value) in cases() {
        for (criterion, selected) in [("", &[0_usize, 1][..]), (" WHERE ID<0", &[][..])] {
            let (before, after) = images(column, value, selected);
            let statement = format!(
                "UPDATE Measurements SET [{}]={expression}{criterion}",
                column.name()
            );
            assert_recovery(
                &before,
                &after,
                NativeUpdateProfile::Ace16RetainedFixedScalarColumnBatch,
                b"TCNI0028",
                || {
                    let image = NativeImage::capture_retained(
                        &mut Cursor::new(&before),
                        SCOPE,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let parsed = parse_query(&statement, ParseLimits::default()).unwrap();
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
}
