//! Original SQL assignments choose scalar21 or LONG17 on the same retained table.
use super::{budget, open};
use crate::editing::{
    offline::test_support::{assert_completion, assert_staged_read_state},
    sql,
};
use std::{fs, io::Cursor};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields,
    fixed_scalars::{self as raw, Column, Scalar},
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected),
        None,
        "first differing image byte"
    );
}

#[test]
fn reviewed_scalar_sql_and_long_control_keep_their_exact_native_families() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "Measurements");
    let mut before = raw::append(&empty, definition);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("measurements.accdb");
    fs::write(&path, &before).unwrap();
    for (step, statement, profile) in [
        (
            0,
            "UPDATE Measurements SET ByteValue=255 WHERE ID=1",
            NativeUpdateProfile::Ace16RetainedFixedScalarRows,
        ),
        (
            1,
            "UPDATE Measurements SET Score=124 WHERE ID=1",
            NativeUpdateProfile::Ace16RetainedRows,
        ),
    ] {
        let after = if step == 0 {
            raw::update(
                &before,
                definition,
                0,
                Column::ByteValue,
                Some(Scalar::Byte(255)),
            )
        } else {
            raw::update_score(&before, definition, 0, Some(124))
        };
        let image = NativeImage::capture_retained(
            &mut Cursor::new(&before),
            CatalogScope::new(2141),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let parsed = parse_query(statement, ParseLimits::default()).unwrap();
        let review =
            sql::plan_recoverable_batch_update(image, parsed.query(), &mut budget(), None, &|| {
                false
            })
            .unwrap();
        assert_eq!(review.backend().intent().profile(), profile);
        assert_image(review.backend().after_bytes(), &after);
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(review, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_staged_read_state(&path, &before);
        let bundle = directory.path().join(format!("journal-{step}"));
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1))
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        session.release().unwrap();
        assert_image(&fs::read(&path).unwrap(), &after);
        before = after;
    }
}
