//! Authored nullable Memo schema over a fresh copy, not an Access-authored index.
use super::{budget, copy};
use std::io::Cursor;
use tablecraft_ace::{
    InspectError, TextIndexSelection, inspect_table_schema, read_table_index,
    read_table_text_index_range,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn nullable_memo_index_profile_uses_long_text_metadata_and_retains_null_keys() {
    let mut original = Cursor::new(copy());
    let scope = CatalogScope::new(1);
    let baseline = read_table_index(
        &mut original,
        scope,
        "USysRibbons",
        "CreatedId",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(baseline.rows().iter().all(|row| row[3] == Value::Null));
    let schema = inspect_table_schema(&mut original, scope, "USysRibbons", &mut budget(), &|| {
        false
    })
    .unwrap();
    let base = schema.definition_page() as usize * 4096;
    let mut bytes = original.into_inner();
    // Independent Jet 4/ACE table-definition geometry. RibbonNotes is the fourth
    // physical column; its two stored NULLs require no changed record bodies.
    let indexes = u32::from_le_bytes(bytes[base + 51..base + 55].try_into().unwrap()) as usize;
    let column = base + 63 + indexes * 12 + 3 * 25;
    assert_eq!(bytes[column], 10);
    bytes[column] = 12;
    bytes[column + 23..column + 25].copy_from_slice(&0_u16.to_le_bytes());
    let mut source = Cursor::new(bytes.clone());
    for selection in [TextIndexSelection::All, TextIndexSelection::Null] {
        let selected = read_table_text_index_range(
            &mut source,
            scope,
            "USysRibbons",
            "CreatedId",
            selection,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(selected.rows(), baseline.rows());
    }
    assert_eq!(source.get_ref(), &bytes);
    // A malformed selected Memo key must fail rather than become a table scan.
    source.get_mut()[785 * 4096 + 480] = 0x7f;
    assert!(matches!(
        read_table_index(
            &mut source,
            scope,
            "USysRibbons",
            "CreatedId",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
}
