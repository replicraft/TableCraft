use super::*;
use support::{put16, put32};
use tablecraft_ace::inspect_table_indexes;

fn wide_schema() -> Vec<u8> {
    let mut file = copy().into_inner();
    // Independent candidate facts: root 726, three physical index count slots,
    // column metadata starts at 99, index definitions occupy bytes 230..522.
    let original = file[726 * 4096..727 * 4096].to_vec();
    let mut logical = vec![0; 99 + 200 * 25];
    logical[..99].copy_from_slice(&original[..99]);
    put16(&mut logical, 41, 200);
    put16(&mut logical, 43, 0);
    put16(&mut logical, 45, 200);
    for number in 0..200 {
        let at = 99 + number * 25;
        logical[at] = 4;
        put16(&mut logical, at + 5, number as u16);
        logical[at + 15] = 1;
        put16(&mut logical, at + 21, (number * 4) as u16);
        put16(&mut logical, at + 23, 4);
        let name: Vec<_> = format!("F{number:03}_{}é🦀", "x".repeat(48))
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        logical.extend((name.len() as u16).to_le_bytes());
        logical.extend(name);
    }
    logical.extend_from_slice(&original[230..522]);
    let length = logical.len();
    put32(&mut logical, 8, length as u32);
    let first_continuation = file.len() / 4096;
    put32(&mut logical, 4, first_continuation as u32);
    file[726 * 4096..727 * 4096].copy_from_slice(&logical[..4096]);
    let chunks = logical[4096..].chunks(4088);
    let count = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        let mut page = vec![0; 4096];
        page[..2].copy_from_slice(&[2, 1]);
        if i + 1 < count {
            put32(&mut page, 4, (first_continuation + i + 1) as u32);
        }
        page[8..8 + chunk.len()].copy_from_slice(chunk);
        file.extend(page);
    }
    file
}

#[test]
fn column_blocks_unicode_names_and_index_definitions_span_continuation_headers() {
    let bytes = wide_schema();
    let mut source = Cursor::new(bytes.clone());
    let schema = inspect_table_schema(
        &mut source,
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns().len(), 200);
    for (number, column) in schema.columns().iter().enumerate() {
        assert_eq!(column.name(), format!("F{number:03}_{}é🦀", "x".repeat(48)));
        assert_eq!(column.number(), number);
        assert_eq!(column.scalar_type(), Some(ScalarType::Int32));
    }
    let indexes = inspect_table_indexes(
        &mut source,
        CatalogScope::new(1),
        "ROULETTE_TB",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        indexes
            .indexes()
            .iter()
            .map(|i| (i.name(), i.columns()[0].number()))
            .collect::<Vec<_>>(),
        vec![("COLOR_ID", 2), ("Number", 1), ("ORDR_NBR", 0)]
    );
    assert_eq!(source.into_inner(), bytes);
}

#[test]
fn damaged_continuations_fail_schema_and_index_inspection_without_partial_metadata() {
    for change in 0..4 {
        let mut bytes = wide_schema();
        let first = 827 * 4096;
        match change {
            0 => bytes[first] = 1,
            1 => put32(&mut bytes, first + 4, 827),
            2 => put32(&mut bytes, first + 4, 0),
            _ => put32(&mut bytes, 726 * 4096 + 8, 261641),
        }
        assert!(
            inspect_table_schema(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        assert!(
            inspect_table_indexes(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "ROULETTE_TB",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}
