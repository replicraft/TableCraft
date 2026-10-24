#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "calculated_memos/fixture.rs"]
mod memo_fixture;
mod support;

use memo_fixture::{FIELD_START, chained_database, database, descriptor, inline, lval, wrapped};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    InspectError, LongTextReadLimits, LongValueReadLimits, LongValueReadSource, StoredCalculation,
    TablePageRequest, inspect_calculation_declaration, read_stored_calculation,
    read_stored_table_page, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 64 * 1024 * 1024)
}

fn read(bytes: &[u8]) -> Result<StoredCalculation, InspectError> {
    read_stored_calculation(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}

fn utf16(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

#[test]
fn cached_memos_resolve_inline_single_and_chained_storage_before_unwrapping() {
    let text = "é🦀".repeat(1000);
    let chained = wrapped(&utf16(&text));
    let first = [(15_u32 << 8).to_le_bytes().as_slice(), &chained[..4000]].concat();
    let second = [0_u32.to_le_bytes().as_slice(), &chained[4000..]].concat();
    let single = wrapped(&utf16("é🦀\t\n"));
    for code in [5, 6] {
        let bytes = database(
            code,
            false,
            &[
                Some(inline(&wrapped(&utf16("ASCII")))),
                Some(descriptor(0x4000_0000, single.len(), 13 << 8)),
                Some(descriptor(0, chained.len(), 14 << 8)),
                None,
                Some(inline(&wrapped(&[]))),
            ],
            &[(13, lval(&single)), (14, lval(&first)), (15, lval(&second))],
        );
        let stored = read(&bytes).unwrap();
        assert_eq!(stored.data_type(), ScalarType::Text);
        assert_eq!(stored.column().storage_length(), 0);
        assert_eq!(stored.column().scalar_type(), None);
        assert_eq!(
            stored.values(),
            [
                Value::Text("ASCII".into()),
                Value::Text("é🦀\t\n".into()),
                Value::Text(text.clone()),
                Value::Null,
                Value::Text(String::new()),
            ]
        );
        let page = read_stored_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 6).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(page.reached_end());
        assert_eq!(page.is_stored_calculation(1), Some(true));
        assert_eq!(page.rows()[2][1], Value::Text(text.clone()));
        assert_eq!(
            read_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::Unsupported)
        );
    }
}

#[test]
fn observed_compression_modes_keep_empty_null_unicode_and_marker_rules() {
    for code in [5, 6] {
        let bytes = database(
            code,
            false,
            &[
                Some(inline(&wrapped(b"\xff\xfeASCII"))),
                Some(inline(&wrapped(&utf16("é🦀")))),
                Some(inline(&wrapped(&[]))),
                None,
            ],
            &[],
        );
        assert_eq!(
            read(&bytes).unwrap().values(),
            [
                Value::Text("ASCII".into()),
                Value::Text("é🦀".into()),
                Value::Text(String::new()),
                Value::Null,
            ]
        );
    }
}

#[test]
fn malformed_late_wrappers_references_and_text_publish_no_cached_snapshot() {
    let valid = Some(inline(&wrapped(&utf16("valid"))));
    let mut unknown = wrapped(&utf16("invalid"));
    unknown[0] = 1;
    let corruptions = [
        inline(&unknown),
        inline(&wrapped(&[0])),
        inline(&wrapped(&[0x00, 0xd8])),
        inline(&[]),
        descriptor(0x4000_0000, 30, 999 << 8),
    ];
    for bad in corruptions {
        assert!(read(&database(6, false, &[valid.clone(), Some(bad)], &[])).is_err());
    }
    let cycle = [(13_u32 << 8).to_le_bytes().as_slice(), b"ab"].concat();
    let bytes = database(
        6,
        false,
        &[valid, Some(descriptor(0, 10, 13 << 8))],
        &[(13, lval(&cycle))],
    );
    assert!(read(&bytes).is_err());
}

#[test]
fn memo_declarations_gate_profiles_geometry_and_flags_even_when_empty() {
    let bytes = database(6, false, &[], &[]);
    assert!(read(&bytes).unwrap().values().is_empty());
    for (offset, value) in [(20, 2), (FIELD_START + 23, 23), (FIELD_START + 16, 0xc1)] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        assert_eq!(read(&changed).err(), Some(InspectError::Unsupported));
    }
    let corrupt = database(6, false, &[Some(inline(&[255]))], &[]);
    let declaration = inspect_calculation_declaration(
        &mut Cursor::new(corrupt),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(declaration.data_type(), ScalarType::Text);
    assert_eq!(declaration.column().storage_length(), 0);
}

#[test]
fn decoded_text_encoded_storage_native_budget_and_cancellation_are_separate() {
    let text = vec![b'x'; 1024 * 1024 + 1];
    let compressed = [b"\xff\xfe".as_slice(), &text].concat();
    let bytes = chained_database(6, false, &compressed);
    assert_eq!(read(&bytes).err(), Some(InspectError::StructureLimit));
    let mut source = LongValueReadSource::new(Cursor::new(&bytes), LongValueReadLimits::default())
        .with_text_limits(LongTextReadLimits::new(2 * 1024 * 1024).unwrap());
    let stored = read_stored_calculation(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        stored.values(),
        [Value::Text(String::from_utf8(text.clone()).unwrap())]
    );

    let bytes = chained_database(
        6,
        false,
        &text.iter().flat_map(|byte| [*byte, 0]).collect::<Vec<_>>(),
    );
    let mut source = LongValueReadSource::new(Cursor::new(&bytes), LongValueReadLimits::default())
        .with_text_limits(LongTextReadLimits::new(2 * 1024 * 1024).unwrap());
    assert_eq!(
        read_stored_calculation(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .err(),
        Some(InspectError::StructureLimit)
    );
    let mut source = LongValueReadSource::new(
        Cursor::new(&bytes),
        LongValueReadLimits::new(4 * 1024 * 1024).unwrap(),
    )
    .with_text_limits(LongTextReadLimits::new(2 * 1024 * 1024).unwrap());
    let stored = read_stored_calculation(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        stored.values(),
        [Value::Text(String::from_utf8(text).unwrap())]
    );

    let bytes = database(6, false, &[Some(inline(&wrapped(&utf16("small"))))], &[]);
    let mut denied = ResourceBudget::new(0, 0, 0);
    assert!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut denied,
            &|| false,
        )
        .is_err()
    );
    let calls = Cell::new(0);
    let cancellation = || {
        calls.set(calls.get() + 1);
        calls.get() > 8
    };
    assert_eq!(
        read_stored_calculation(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &cancellation,
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    let mut source = LongValueReadSource::new(Cursor::new(&bytes), LongValueReadLimits::default())
        .with_text_limits(LongTextReadLimits::new(1024 * 1024).unwrap());
    assert_eq!(
        read_stored_calculation(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .values(),
        [Value::Text("small".into())]
    );
}
