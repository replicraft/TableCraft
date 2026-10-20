use super::{budget, copy};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, read_table, read_table_index,
    read_table_index_projection,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn registered_text_index_matches_reference_order_and_exact_stored_rows() {
    let bytes = copy();
    let mut source = Cursor::new(bytes.clone());
    let scope = CatalogScope::new(1);
    let indexed = read_table_index(
        &mut source,
        scope,
        "TAG_GRP_TB",
        "TAG_GRP_NME",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    // Independently recorded Jackcess cursor order, including OLE/Office and
    // VBA/Visual Basic distinctions. This is not an Access SQL comparison oracle.
    let expected = [
        1, 2, 3, 4, 5, 6, 7, 9, 8, 10, 11, 12, 13, 14, 15, 18, 17, 16, 19,
    ];
    assert_eq!(
        indexed
            .rows()
            .iter()
            .map(|row| match row[0] {
                Value::Int32(id) => id,
                _ => panic!("ID carrier"),
            })
            .collect::<Vec<_>>(),
        expected
    );
    let sequential =
        read_table(&mut source, scope, "TAG_GRP_TB", &mut budget(), &|| false).unwrap();
    for row in indexed.rows() {
        assert!(sequential.rows().iter().any(|stored| stored == row));
    }
    let projected = read_table_index_projection(
        &mut source,
        scope,
        "TAG_GRP_TB",
        "TAG_GRP_NME",
        IndexProjection {
            columns: &["TAG_GRP_ID"],
            selection: IndexProjectionSelection::All,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        projected
            .rows()
            .iter()
            .map(|row| row[0].clone())
            .collect::<Vec<_>>(),
        expected.into_iter().map(Value::Int32).collect::<Vec<_>>()
    );
    assert_eq!(source.into_inner(), bytes);
}

#[test]
fn text_key_row_disagreement_fails_even_when_text_is_not_projected() {
    let mut bytes = copy();
    // Registered single-leaf text index. Its first entry is never prefix-compressed.
    let page = &mut bytes[740 * 4096..741 * 4096];
    assert_eq!(page[0], 4);
    let ends: Vec<_> = page[27..480]
        .iter()
        .enumerate()
        .flat_map(|(block, byte)| {
            (0..8).filter_map(move |bit| (byte & (1 << bit) != 0).then_some(block * 8 + bit))
        })
        .take(2)
        .collect();
    let second_row: [u8; 4] = page[480 + ends[1] - 4..480 + ends[1]].try_into().unwrap();
    page[480 + ends[0] - 4..480 + ends[0]].copy_from_slice(&second_row);
    for projected in [false, true] {
        let mut source = Cursor::new(bytes.clone());
        let result = if projected {
            read_table_index_projection(
                &mut source,
                CatalogScope::new(1),
                "TAG_GRP_TB",
                "TAG_GRP_NME",
                IndexProjection {
                    columns: &["TAG_GRP_ID"],
                    selection: IndexProjectionSelection::All,
                },
                &mut budget(),
                &|| false,
            )
        } else {
            read_table_index(
                &mut source,
                CatalogScope::new(1),
                "TAG_GRP_TB",
                "TAG_GRP_NME",
                &mut budget(),
                &|| false,
            )
        };
        assert!(matches!(result, Err(InspectError::Malformed)));
        assert_eq!(source.into_inner(), bytes);
    }
}
