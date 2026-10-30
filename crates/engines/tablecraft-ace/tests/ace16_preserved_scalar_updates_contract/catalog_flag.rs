//! Separate HAS_COMPLEX source derivative; original constructors remain frozen.
use super::{SCOPE, budget, model, support};
use std::io::Cursor;
use tablecraft_ace::{
    inspect_catalog, inspect_table_schema, read_attachment_column, read_stored_int32_calculation,
    read_table_projection,
};
use tablecraft_core::values::Value;

pub(super) fn source() -> model::Case {
    let mut case = model::source();
    let slot = 4 * 4096 + 14 + 2 * 2;
    let start = u16::from_le_bytes(case.before[slot..slot + 2].try_into().unwrap()) as usize;
    let flags = 4 * 4096 + start + 12;
    assert_eq!(&case.before[flags..flags + 4], &[0; 4]);
    assert_eq!(&case.expected[flags..flags + 4], &[0; 4]);
    support::put32(&mut case.before, flags, 0x0004_0000);
    support::put32(&mut case.expected, flags, 0x0004_0000);
    case
}

#[test]
fn qualify_preserved_boolean_genuine_complex_catalog_flag_source_before_writer() {
    let case = source();
    let differences = case
        .before
        .iter()
        .zip(&case.expected)
        .enumerate()
        .filter_map(|(offset, (before, after))| {
            (before != after).then_some((offset, before ^ after))
        })
        .collect::<Vec<_>>();
    assert_eq!(differences, [(model::BOOLEAN_PATCH_OFFSET, 4)]);

    for (bytes, first) in [(&case.before, false), (&case.expected, true)] {
        let catalog =
            inspect_catalog(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
        let schema = inspect_table_schema(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let origin = catalog
            .origins()
            .iter()
            .find(|origin| origin.object_id() == schema.object_id())
            .unwrap();
        assert_eq!(origin.flags(), 0x0004_0000);
        let selected = read_table_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &["ID", "Yes_Field"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(selected.rows()[0], [Value::Int32(1), Value::Boolean(first)]);

        let attachments = read_attachment_column(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            "Attachment_Field",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            attachments
                .links()
                .parents()
                .iter()
                .map(|parent| (parent.key(), parent.child_count()))
                .collect::<Vec<_>>(),
            [(Some(10), 2), (Some(20), 1), (Some(30), 0)]
        );
        let cached = read_stored_int32_calculation(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            "Calculated",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(cached.values(), &[Some(2), Some(4), Some(6)]);
        let payloads = read_table_projection(
            &mut Cursor::new(bytes),
            SCOPE,
            "Items",
            &["Long_Text", "OLE_Object_Field", "Hyperlink_Field"],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            payloads.rows()[0],
            [
                Value::Text("memo".to_owned()),
                Value::Binary(vec![0, 255, 7]),
                Value::Text("label#https://invalid.test/#".to_owned())
            ]
        );
    }
}
