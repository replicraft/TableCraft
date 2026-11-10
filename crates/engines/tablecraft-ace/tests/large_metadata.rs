use std::io::Cursor;
use tablecraft_ace::{
    InspectError, inspect_catalog, inspect_complex_schema, read_saved_query_definition,
    read_table_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    complex_schema,
    large_tables::append_records,
    mutation_fixture, put32, record,
    saved_queries::{self, Fragment},
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn large_catalog() -> Vec<u8> {
    let mut bytes = mutation_fixture::database();
    let rows = (0..16_385)
        .map(|index| {
            let name = format!("Object{index}");
            let name = [b"\xff\xfe".as_slice(), name.as_bytes()].concat();
            (0, record(index + 100, 0, 5, 0, &name))
        })
        .collect();
    append_records(&mut bytes, 2, 3, rows);
    bytes
}

#[test]
fn complete_catalog_retains_late_objects_and_rejects_late_corruption_or_budget_failure() {
    let bytes = large_catalog();
    let scope = CatalogScope::new(1);
    let result =
        inspect_catalog(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false).unwrap();
    assert_eq!(result.catalog().objects().len(), 16_385);
    assert_eq!(
        result.catalog().objects()[16_384].name().as_str(),
        "Object16384"
    );
    assert_eq!(result.origins()[16_384].native_id(), 16_484);

    let mut short = ResourceBudget::new(128 * 1024 * 1024, 16_384, 200_000_000);
    assert!(matches!(
        inspect_catalog(&mut Cursor::new(&bytes), scope, &mut short, &|| false),
        Err(InspectError::Limit(_))
    ));
    let mut malformed = bytes;
    let last = malformed.len() - 4096;
    put32(&mut malformed, last + 4, 10);
    assert!(matches!(
        inspect_catalog(&mut Cursor::new(&malformed), scope, &mut budget(), &|| {
            false
        }),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn saved_query_definition_finds_its_fragment_after_sixteen_thousand_unrelated_rows() {
    let mut fragments = (0..16_384)
        .map(|_| Fragment {
            object: Some(saved_queries::EMPTY_ID),
            attribute: Some(99),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    fragments.push(Fragment {
        object: Some(saved_queries::QUERY_ID),
        attribute: Some(99),
        expression: Some("late fragment"),
        ..Default::default()
    });
    let bytes = saved_queries::database_with(&fragments);
    let result = read_saved_query_definition(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Saved",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(result.rows()[0].expression(), Some("late fragment"));
}

#[test]
fn complex_mapping_resolution_does_not_ignore_a_late_matching_declaration() {
    let mut bytes = complex_schema::database();
    let mut rows = (0..16_384)
        .map(|index| (0, complex_schema::mapping(index + 100, 10, 20, 17)))
        .collect::<Vec<_>>();
    rows.push((0, complex_schema::mapping(7, 10, 20, 17)));
    append_records(&mut bytes, 14, 15, rows);
    let result = inspect_complex_schema(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.columns().len(), 1);
    assert_eq!(result.columns()[0].type_id(), 7);
    assert_eq!(result.columns()[0].flat_table().definition_page(), 20);
}

fn relationship_row(name: &str) -> Vec<u8> {
    let mut row = 8_u16.to_le_bytes().to_vec();
    row.extend([0; 12]);
    let mut offsets = vec![row.len() as u16];
    for text in [name, "Items", "Items", "Id", "Score"] {
        row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(row.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(5_u16.to_le_bytes());
    row.push(255);
    row
}

#[test]
fn selected_relationship_metadata_reads_every_row_and_checks_complete_eof() {
    let mut bytes = mutation_fixture::database();
    let rows = (0..16_385)
        .map(|index| (0, relationship_row(&format!("Relation{index}"))))
        .collect();
    append_records(&mut bytes, 7, 8, rows);
    let result = read_table_projection(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "MSysRelationships",
        &["szRelationship"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.rows().len(), 16_385);
    assert_eq!(result.rows()[16_384], [Value::Text("Relation16384".into())]);
    put32(&mut bytes, 7 * 4096 + 16, 16_386);
    assert!(matches!(
        read_table_projection(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "MSysRelationships",
            &["szRelationship"],
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Malformed)
    ));
}
