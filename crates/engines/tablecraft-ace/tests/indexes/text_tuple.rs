use super::{budget, copy};
use std::io::Cursor;
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, inspect_table_indexes, read_table,
    read_table_index, read_table_index_projection,
};
use tablecraft_model::CatalogScope;

#[test]
fn registered_numeric_text_tuple_matches_complete_rows_and_projection() {
    let original = copy();
    let mut source = Cursor::new(original.clone());
    let scope = CatalogScope::new(1);
    let indexed = read_table_index(
        &mut source,
        scope,
        "MSysObjects",
        "ParentIdName",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let sequential =
        read_table(&mut source, scope, "MSysObjects", &mut budget(), &|| false).unwrap();
    assert_eq!(indexed.rows().len(), 146);
    for row in indexed.rows() {
        assert!(sequential.rows().contains(row));
    }
    let projected = read_table_index_projection(
        &mut source,
        scope,
        "MSysObjects",
        "ParentIdName",
        IndexProjection {
            columns: &["Id"],
            selection: IndexProjectionSelection::All,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.rows().len(), 146);
    for (full, projected) in indexed.rows().iter().zip(projected.rows()) {
        assert_eq!(full[..1], projected[..]);
    }
    assert_eq!(source.get_ref(), &original);
}

#[test]
fn compound_row_disagreement_fails_even_when_both_key_fields_are_omitted() {
    let mut source = Cursor::new(copy());
    let metadata = inspect_table_indexes(
        &mut source,
        CatalogScope::new(1),
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut page = metadata
        .indexes()
        .iter()
        .find(|index| index.name() == "ParentIdName")
        .unwrap()
        .root_page() as usize;
    let mut image = source.into_inner();
    loop {
        let bytes = &mut image[page * 4096..(page + 1) * 4096];
        // Independent Jet 4/ACE bitmap and first-entry geometry. The first
        // entry is complete; later entries may share its prefix.
        let end = bytes[27..480]
            .iter()
            .enumerate()
            .find_map(|(block, byte)| {
                (0..8)
                    .find(|bit| byte & (1 << bit) != 0)
                    .map(|bit| block * 8 + bit)
            })
            .unwrap();
        if bytes[0] == 3 {
            page = u32::from_be_bytes(bytes[480 + end - 4..480 + end].try_into().unwrap()) as usize;
        } else {
            assert_eq!(bytes[0], 4);
            // Change only the referenced row slot, retaining the full key.
            bytes[480 + end - 1] ^= 1;
            break;
        }
    }
    for projected in [false, true] {
        let mut source = Cursor::new(image.clone());
        let result = if projected {
            read_table_index_projection(
                &mut source,
                CatalogScope::new(1),
                "MSysObjects",
                "ParentIdName",
                IndexProjection {
                    columns: &["Id"],
                    selection: IndexProjectionSelection::All,
                },
                &mut budget(),
                &|| false,
            )
        } else {
            read_table_index(
                &mut source,
                CatalogScope::new(1),
                "MSysObjects",
                "ParentIdName",
                &mut budget(),
                &|| false,
            )
        };
        assert!(matches!(result, Err(InspectError::Malformed)));
        assert_eq!(source.get_ref(), &image);
    }
}
