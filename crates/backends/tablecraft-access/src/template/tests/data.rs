use super::*;
use std::cell::Cell;
use tablecraft_query::{QueryError, Source, prepare};
use tablecraft_sql::parse_query;

const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="text/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/metadata.xml" ContentType="application/xml"/><Override PartName="/data.xml" ContentType="application/xml"/></Types>"#;
const ROOT: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="start" Type="http://schemas.microsoft.com/office/access/2005/04/template/start" Target="template.xml"/></Relationships>"#;
const OBJECTS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="table" Type="http://schemas.microsoft.com/office/access/2005/04/template/object" Target="definition.xml"/></Relationships>"#;
const LINKS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="metadata" Type="http://schemas.microsoft.com/office/access/2005/04/template/object-metadata" Target="metadata.xml"/><Relationship Id="data" Type="http://schemas.microsoft.com/office/access/2005/04/template/table-data" Target="data.xml"/></Relationships>"#;
const METADATA: &str = r#"<m:AccessObject xmlns:m="http://schemas.microsoft.com/office/access/2005/04/template/object-metadata"><m:Type>Table</m:Type><m:Name>People</m:Name><m:NameMap><unknown>inert</unknown></m:NameMap></m:AccessObject>"#;
const DATA: &str = r#"<root xmlns:x="http://www.w3.org/2001/XMLSchema" xmlns:od="urn:schemas-microsoft-com:officedata">
<x:schema>
<x:element name="dataroot"><x:complexType><x:sequence><x:element ref="People" minOccurs="0" maxOccurs="unbounded"/></x:sequence></x:complexType></x:element>
<x:element name="People"><x:complexType><x:sequence>
<x:element name="ID" type="x:int"/>
<x:element name="Full_x0020_Name" minOccurs="0"><x:simpleType><x:restriction base="x:string"><x:maxLength value="20"/></x:restriction></x:simpleType></x:element>
<x:element name="Flag" type="x:boolean"/>
<x:element name="Price" type="x:decimal"/>
<x:element name="Stamp" type="x:dateTime"/>
<x:element name="Payload" type="x:base64Binary" nillable="true"/>
</x:sequence></x:complexType></x:element>
</x:schema>
<dataroot xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<People><ID>1</ID><Full_x0020_Name> Jo&amp;🦀 </Full_x0020_Name><Flag>true</Flag><Price>1.2500</Price><Stamp>2025-01-01T12:00:00</Stamp><Payload>AQI=</Payload></People>
<People><ID>2</ID><Flag>0</Flag><Price>-0.25</Price><Stamp>1899-12-29T06:00:00</Stamp><Payload xsi:nil="true"/></People>
</dataroot></root>"#;

fn fixture(data: &str, metadata: &str, objects: &str, links: &str) -> TemplatePackage {
    open(&package(
        &[
            ("[Content_Types].xml", TYPES),
            ("_rels/.rels", ROOT),
            ("template.xml", "<inert/ >"),
            ("_rels/template.xml.rels", objects),
            (
                "definition.xml",
                "arbitrary object body: never parsed or executed",
            ),
            ("_rels/definition.xml.rels", links),
            ("metadata.xml", metadata),
            ("data.xml", data),
        ],
        CompressionMethod::Deflated,
    ))
    .unwrap()
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100000, 1000000)
}

#[test]
fn reusable_queries_join_group_filter_and_order_template_rows() {
    let package = fixture(DATA, METADATA, OBJECTS, LINKS);
    let source = TemplateSource::new(&package, budget(), &|| false).unwrap();
    for (sql, expected) in [
        (
            "SELECT ID FROM People WHERE Flag ORDER BY ID",
            vec![vec![Value::Int32(1)]],
        ),
        (
            "SELECT Count(*),Sum(ID) FROM People",
            vec![vec![Value::Int64(2), Value::Int64(3)]],
        ),
        (
            "SELECT A.ID FROM People AS A INNER JOIN People AS B ON A.ID=B.ID WHERE B.ID=2",
            vec![vec![Value::Int32(2)]],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            expected
        );
    }
}

#[test]
fn binding_stops_before_data_even_when_records_are_invalid() {
    for invalid in [
        DATA.replace("<ID>2</ID>", "<ID>bad</ID>"),
        DATA.replace("<ID>2</ID>", "<ID>"),
        DATA.replace("<ID>2</ID>", "<!DOCTYPE evil>"),
    ] {
        let package = fixture(&invalid, METADATA, OBJECTS, LINKS);
        let source = TemplateSource::new(&package, budget(), &|| false).unwrap();
        let parsed = parse_query("SELECT ID FROM People", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert!(plan.execute(&source, &[], Default::default()).is_err());
        let mut calls = 0;
        assert!(
            source
                .scan("People", &mut |_| {
                    calls += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(calls, 0);
    }
}

#[test]
fn parsing_and_reading_enforce_budgets_and_each_observed_cancellation_point() {
    let package = fixture(DATA, METADATA, OBJECTS, LINKS);
    let visits = Cell::new(0usize);
    let invoke = |stop| {
        visits.set(0);
        package.read_table("People", &mut budget(), &|| {
            visits.set(visits.get() + 1);
            visits.get() == stop
        })
    };
    assert!(invoke(usize::MAX).is_ok());
    let count = visits.get();
    for point in 1..=count {
        assert!(matches!(
            invoke(point),
            Err(TemplateError::Read(ReadError::Cancelled))
        ));
    }
    let mut bytes = ResourceBudget::new(16, 100, 1000000);
    assert!(matches!(
        package.read_table("People", &mut bytes, &|| false),
        Err(TemplateError::Read(ReadError::Limit(_)))
    ));
    let mut rows = ResourceBudget::new(64 * 1024 * 1024, 1, 1000000);
    assert!(matches!(
        package.read_table("People", &mut rows, &|| false),
        Err(TemplateError::Read(ReadError::Limit(_)))
    ));
    let source = TemplateSource::new(&package, budget(), &|| false).unwrap();
    let mut visits = 0;
    assert_eq!(
        source.scan("People", &mut |_| {
            visits += 1;
            Err(QueryError::Cardinality)
        }),
        Err(QueryError::Cardinality)
    );
    assert_eq!(visits, 1);
}

#[test]
fn reentrant_schema_callbacks_fail_without_poisoning_cached_metadata() {
    let package = fixture(DATA, METADATA, OBJECTS, LINKS);
    let active: Cell<Option<&TemplateSource<'_>>> = Cell::new(None);
    let cancelled = Cell::new(false);
    let calls = Cell::new(0);
    let callback = || {
        if let Some(source) = active.get() {
            calls.set(calls.get() + 1);
            assert_eq!(source.columns("People").err(), Some(QueryError::Source));
        }
        cancelled.get()
    };
    let source = TemplateSource::new(&package, budget(), &callback).unwrap();
    active.set(Some(&source));
    assert_eq!(source.columns("People").unwrap().len(), 6);
    assert!(calls.get() > 0);
    cancelled.set(true);
    assert_eq!(source.columns("People").err(), Some(QueryError::Cancelled));
}

#[test]
fn interrupted_schema_discovery_can_be_explicitly_reused_without_caching_cancellation() {
    let package = fixture(DATA, METADATA, OBJECTS, LINKS);
    let visits = Cell::new(0usize);
    let stop = Cell::new(None);
    let callback = || {
        visits.set(visits.get() + 1);
        stop.get() == Some(visits.get())
    };
    let source = TemplateSource::new(&package, budget(), &callback).unwrap();
    visits.set(0);
    stop.set(Some(4));
    assert_eq!(source.columns("People").err(), Some(QueryError::Cancelled));
    stop.set(None);
    assert_eq!(source.columns("People").unwrap().len(), 6);
}
mod named_types;
