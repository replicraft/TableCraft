//! Default collection metadata must preserve independently qualified ACE2010 reads.
use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_ace::{
    FileFormat, InspectError, inspect_attachment_declarations, inspect_header,
    read_multivalue_column,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, SourceFailure, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{attachment_modern, complex_links, complex_schema};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 4_000_000)
}

fn execute(source: &dyn Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(query.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

fn ace2010_multivalue() -> Vec<u8> {
    // This exact raw variable-reference geometry is already qualified for code3
    // by tablecraft-ace/tests/complex_variable_references.rs. The profile change
    // does not invent an attachment layout or claim native Access execution.
    let mut bytes = complex_links::variable::populated();
    bytes[20] = 3;
    assert_eq!(
        inspect_header(&mut Cursor::new(&bytes), &mut budget(), &|| false)
            .unwrap()
            .format(),
        FileFormat::Ace2010
    );
    let values = read_multivalue_column(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        values.values(0).unwrap().cloned().collect::<Vec<_>>(),
        vec![Value::Int32(100), Value::Int32(101)]
    );
    bytes
}

#[test]
fn ace2010_collection_metadata_preserves_scalar_and_qualified_multivalue_queries() {
    let bytes = ace2010_multivalue();
    let scalar = NativeSource::new(Cursor::new(&bytes), CatalogScope::new(2), budget(), &|| {
        false
    })
    .unwrap();
    let expected = vec![
        vec![Value::Int32(1)],
        vec![Value::Int32(2)],
        vec![Value::Int32(3)],
    ];
    assert_eq!(execute(&scalar, "SELECT Id FROM Items").unwrap(), expected);
    let values_only =
        NativeSource::new(Cursor::new(&bytes), CatalogScope::new(3), budget(), &|| {
            false
        })
        .unwrap()
        .with_multivalues();
    let expanded = execute(&values_only, "SELECT Id, Score.Value FROM Items").unwrap();
    assert_eq!(
        expanded,
        vec![
            vec![Value::Int32(1), Value::Int32(100)],
            vec![Value::Int32(1), Value::Int32(101)],
            vec![Value::Int32(2), Value::Int32(200)],
            vec![Value::Int32(3), Value::Null],
        ]
    );
    let collections =
        NativeSource::new(Cursor::new(&bytes), CatalogScope::new(4), budget(), &|| {
            false
        })
        .unwrap()
        .with_collections();
    assert_eq!(
        execute(&collections, "SELECT Id FROM Items").unwrap(),
        expected
    );
    assert!(collections.collection_members("Items").unwrap().is_empty());
    assert_eq!(
        execute(&collections, "SELECT Id, Score.Value FROM Items").unwrap(),
        expanded
    );
    assert_eq!(
        execute(&collections, "SELECT Score.FileName FROM Items").unwrap_err(),
        QueryError::UnknownName
    );
}

#[test]
fn ace2010_unqualified_attachments_remain_unavailable_without_losing_scalar_reads() {
    // The existing raw factory authors these exact modern attachment descriptors
    // for the requested code. ACE2010 attachment admission deliberately refuses
    // them; collection metadata must not reinterpret them as a qualified profile.
    let bytes = attachment_modern::database(3);
    assert_eq!(
        inspect_header(&mut Cursor::new(&bytes), &mut budget(), &|| false)
            .unwrap()
            .format(),
        FileFormat::Ace2010
    );
    assert!(matches!(
        inspect_attachment_declarations(
            &mut Cursor::new(&bytes),
            CatalogScope::new(5),
            "Items",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    let scalar = NativeSource::new(Cursor::new(&bytes), CatalogScope::new(6), budget(), &|| {
        false
    })
    .unwrap();
    let expected = execute(&scalar, "SELECT Id FROM Items").unwrap();
    let collections =
        NativeSource::new(Cursor::new(&bytes), CatalogScope::new(7), budget(), &|| {
            false
        })
        .unwrap()
        .with_collections();
    assert_eq!(
        execute(&collections, "SELECT Id FROM Items").unwrap(),
        expected
    );
    assert!(collections.collection_members("Items").unwrap().is_empty());
    assert_eq!(
        execute(&collections, "SELECT Score.FileName FROM Items").unwrap_err(),
        QueryError::UnknownName
    );
}

#[test]
fn ace2010_collection_metadata_does_not_suppress_corrupt_resolved_mappings() {
    let mut bytes = complex_schema::with_mappings(vec![complex_schema::mapping(7, 99, 20, 17)]);
    bytes[20] = 3;
    let collections =
        NativeSource::new(Cursor::new(bytes), CatalogScope::new(8), budget(), &|| {
            false
        })
        .unwrap()
        .with_collections();
    assert_eq!(
        execute(&collections, "SELECT Id FROM Items").unwrap_err(),
        QueryError::SourceFailure(SourceFailure::InvalidData)
    );
}
