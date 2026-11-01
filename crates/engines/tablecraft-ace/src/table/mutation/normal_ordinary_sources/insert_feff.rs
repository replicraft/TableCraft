//! Independent raw FEFF source reads and alias observations, never INSERT plans.
use crate::{
    LongValueCheckStatus, NativeImage, PayloadPresence, check_table_long_values, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(159);
const ESCAPED_ROW: [u8; 26] = [
    3, 0, 24, 0, 0, 0, 255, 254, 0, 255, 254, 110, 0, 101, 0, 119, 0, 17, 0, 17, 0, 6, 0, 2, 0, 3,
];
const ALIAS_ROW: [u8; 23] = [
    3, 0, 24, 0, 0, 0, 255, 254, 110, 0, 101, 0, 119, 0, 14, 0, 14, 0, 6, 0, 2, 0, 3,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_view(raw: &[u8], definition: u32, new_text: Option<&str>) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.columns().len(), 3);
    let fields = table.schema().columns();
    assert_eq!(
        (
            fields[1].native_type(),
            fields[1].flags(),
            fields[1].extended_flags(),
            fields[1].storage_length()
        ),
        (10, 2, 0, 80)
    );
    let mut expected = vec![vec![
        Value::Int32(23),
        Value::Text("unchanged sentinel".into()),
        Value::Binary(vec![199, 0, 255]),
    ]];
    if let Some(text) = new_text {
        expected.push(vec![
            Value::Int32(24),
            Value::Text(text.into()),
            Value::Null,
        ]);
    }
    assert_eq!(table.rows(), expected);
    let lval = check_table_long_values(
        &mut image.reader(),
        SCOPE,
        "Sentinel",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(lval.rows(), 1 + usize::from(new_text.is_some()));
    assert_eq!(lval.columns().len(), 1);
    let bytes = &lval.columns()[0];
    assert_eq!(bytes.status(), LongValueCheckStatus::Checked);
    assert_eq!(
        (bytes.values().nulls(), bytes.values().inline()),
        (usize::from(new_text.is_some()), 1)
    );
    assert_eq!(
        (
            bytes.owned_pages(),
            bytes.candidate_pages(),
            bytes.owned_rows(),
            bytes.distinct_chunks()
        ),
        (0, 0, 0, 0)
    );
    let coverage = image
        .inspect_table_macro_coverage(table.schema().object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.table_extra().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    // Existing read/capture capability only: no insertion request is prepared.
    image
        .check_native_ordinary_fitted_table("Sentinel", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires six independently constructed FEFF source views"]
fn escaped_leading_feff_survives_and_unescaped_text_is_explicitly_a_reader_alias() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_INSERT_FEFF_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, definition, data, pin) in [
        (
            5,
            107,
            110,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        ),
        (
            6,
            125,
            127,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        ),
    ] {
        let prefix = format!("h{header}-insert-feff");
        let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
        assert_eq!(digest(&before), pin);
        let escaped = fs::read(directory.join(format!("{prefix}-escaped.accdb"))).unwrap();
        let alias = fs::read(directory.join(format!("{prefix}-unescaped-alias.accdb"))).unwrap();
        // No serde dependency or production format constants supply this oracle.
        for (actual, start, free, record) in [
            (&escaped, 4004_usize, 3986_u16, ESCAPED_ROW.as_slice()),
            (&alias, 4007_usize, 3989_u16, ALIAS_ROW.as_slice()),
        ] {
            let base = data * 4096;
            let count = definition as usize * 4096 + 16;
            assert_eq!(&before[count..count + 4], &[1, 0, 0, 0]);
            assert_eq!(&before[base + 2..base + 4], &[174, 15]);
            assert_eq!(&before[base + 12..base + 16], &[1, 0, 190, 15]);
            assert_eq!(&before[base + 16..base + 18], &[0, 0]);
            assert!(
                before[base + start..base + 4030]
                    .iter()
                    .all(|byte| *byte == 0)
            );
            let mut literal = before.clone();
            literal[count..count + 4].copy_from_slice(&2_u32.to_le_bytes());
            literal[base + 2..base + 4].copy_from_slice(&free.to_le_bytes());
            literal[base + 12..base + 14].copy_from_slice(&2_u16.to_le_bytes());
            literal[base + 16..base + 18].copy_from_slice(&(start as u16).to_le_bytes());
            literal[base + start..base + 4030].copy_from_slice(record);
            assert_eq!(actual, &literal);
            assert_eq!(
                &actual[base + 4030..base + 4096],
                &before[base + 4030..base + 4096]
            );
        }
        cases.push((definition, before, escaped, alias));
    }
    // All original pins and complete literal images are checked before readers.
    for (definition, before, escaped, alias) in cases {
        read_view(&before, definition, None);
        read_view(&escaped, definition, Some("\u{feff}new"));
        read_view(&alias, definition, Some("new"));
    }
}
