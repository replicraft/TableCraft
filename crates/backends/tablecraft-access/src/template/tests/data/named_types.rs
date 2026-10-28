use super::*;
use std::cell::Cell;
use tablecraft_query::{QueryError, Source, prepare};
use tablecraft_sql::parse_query;

fn document(types: &str, fields: &str, rows: &str) -> String {
    format!(
        r#"<root xmlns:x="http://www.w3.org/2001/XMLSchema" xmlns:od="urn:schemas-microsoft-com:officedata" xmlns:e="urn:external">
<x:schema>{types}
<x:element name="dataroot"><x:complexType><x:sequence><x:element ref="People" minOccurs="0" maxOccurs="unbounded"/></x:sequence></x:complexType></x:element>
<x:element name="People"><x:complexType><x:sequence>{fields}</x:sequence></x:complexType></x:element>
</x:schema><dataroot xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">{rows}</dataroot></root>"#
    )
}

fn named(name: &str, base: &str, facets: &str) -> String {
    format!(
        r#"<x:simpleType name="{name}"><x:restriction base="{base}">{facets}</x:restriction></x:simpleType>"#
    )
}

fn read(data: &str) -> Result<TemplateTable, TemplateError> {
    fixture(data, METADATA, OBJECTS, LINKS).read_table("People", &mut budget(), &|| false)
}

#[test]
fn nullable_named_fields_and_bad_row_suffix_keep_schema_only_binding_and_buffered_failure() {
    let types = named("Label", "x:string", r#"<x:maxLength value="4"/>"#);
    let fields = r#"<x:element name="Name" type="Label" minOccurs="0" nillable="true"/>"#;
    let data = document(
        &types,
        fields,
        "<People/><People><Name xsi:nil=\"true\"/></People>",
    );
    assert_eq!(
        read(&data).unwrap().rows(),
        [vec![Value::Null], vec![Value::Null]]
    );
    let invalid = document(&types, fields, "<People><Name>longer</Name></People>");
    let package = fixture(&invalid, METADATA, OBJECTS, LINKS);
    let source = TemplateSource::new(&package, budget(), &|| false).unwrap();
    let parsed = parse_query("SELECT Name FROM People", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    assert!(prepared.execute(&source, &[], Default::default()).is_err());
    let mut visits = 0;
    assert!(
        source
            .scan("People", &mut |_| {
                visits += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(visits, 0);
}

#[test]
fn resolution_consumes_shared_work_and_allocation_and_cancellation_does_not_poison_retry() {
    let types = [named("A", "B", ""), named("B", "x:int", "")].concat();
    let data = document(
        &types,
        r#"<x:element name="ID" type="A"/>"#,
        "<People><ID>1</ID></People>",
    );
    let package = fixture(&data, METADATA, OBJECTS, LINKS);
    let stopped = Cell::new(false);
    let cancelled = || stopped.get();
    let source = TemplateSource::new(&package, budget(), &cancelled).unwrap();
    stopped.set(true);
    assert!(matches!(
        source.columns("People"),
        Err(QueryError::Cancelled)
    ));
    stopped.set(false);
    assert_eq!(source.columns("People").unwrap().len(), 1);
    let mut rows = Vec::new();
    source
        .scan("People", &mut |row| {
            rows.push(row.to_vec());
            Ok(())
        })
        .unwrap();
    assert_eq!(rows, [vec![Value::Int32(1)]]);
}
