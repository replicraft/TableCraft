//! SQL literals retain exact GUID/Decimal values through reviewed native saves.
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
    exact_values as raw, fields,
};

const TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const REPLACEMENT_TOKEN: [u8; 16] = [
    0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
];
const AMOUNT: i128 = 90_071_992_547_409_931_234;
const REPLACEMENT_AMOUNT: i128 = -90_071_992_547_409_934_321;

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
fn reviewed_guid_decimal_sql_and_long_control_preserve_exact_values_and_families() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "ExactValues");
    let mut before = raw::append(&empty, definition, Some(TOKEN), Some(AMOUNT));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("exact-values.accdb");
    fs::write(&path, &before).unwrap();

    for (step, statement) in [
        "UPDATE ExactValues SET Token={GUID {fedcba98-7654-3210-0123-456789abcdef}} WHERE ID=1",
        "UPDATE ExactValues SET Token=Null WHERE ID=1",
        "UPDATE ExactValues SET Token={GUID {00112233-4455-6677-8899-aabbccddeeff}} WHERE ID=1",
        "UPDATE ExactValues SET Amount=CDec('-9007199254740993.4321') WHERE ID=1",
        "UPDATE ExactValues SET Amount=Null WHERE ID=1",
        "UPDATE ExactValues SET Amount=CDec('9007199254740993.1234') WHERE ID=1",
        "UPDATE ExactValues SET Score=124 WHERE ID=1",
    ]
    .into_iter()
    .enumerate()
    {
        let after = match step {
            0 => raw::update_guid(&before, definition, 0, Some(REPLACEMENT_TOKEN)),
            1 => raw::update_guid(&before, definition, 0, None),
            2 => raw::update_guid(&before, definition, 0, Some(TOKEN)),
            3 => raw::update_decimal(&before, definition, 0, Some(REPLACEMENT_AMOUNT)),
            4 => raw::update_decimal(&before, definition, 0, None),
            5 => raw::update_decimal(&before, definition, 0, Some(AMOUNT)),
            6 => raw::update_score(&before, definition, 0, Some(124)),
            _ => unreachable!("each SQL step has one independent image transformation"),
        };
        let image = NativeImage::capture_retained(
            &mut Cursor::new(&before),
            CatalogScope::new(2142),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let parsed = parse_query(statement, ParseLimits::default()).unwrap();
        let reviewed =
            sql::plan_recoverable_batch_update(image, parsed.query(), &mut budget(), None, &|| {
                false
            })
            .unwrap_or_else(|error| panic!("SQL step {step}: {error:?}"));
        let profile = if step == 6 {
            NativeUpdateProfile::Ace16RetainedRows
        } else {
            NativeUpdateProfile::Ace16RetainedWideValueRows
        };
        assert_eq!(reviewed.backend().intent().profile(), profile);
        assert_image(reviewed.backend().after_bytes(), &after);
        let staged = open(&path)
            .stage_reviewed_update_with_budgets(reviewed, &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_staged_read_state(&path, &before);
        let bundle = directory.path().join(format!("journal-{step}"));
        let (owner, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(
            (receipt.matched_rows(), receipt.changed_rows()),
            (Some(1), Some(1)),
        );
        assert_eq!(receipt.schema_change(), None);
        assert_completion(&receipt);
        owner.release().unwrap();
        assert!(!bundle.exists());
        assert_image(&fs::read(&path).unwrap(), &after);
        before = after;
    }
}
