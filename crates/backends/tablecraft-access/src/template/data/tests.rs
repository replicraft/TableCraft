use super::super::{
    TemplateSource,
    tests::{open, package},
};
use super::*;
use std::cell::Cell;
use tablecraft_core::values::{Decimal, LegacyDateTime};
use tablecraft_query::{QueryError, Source, prepare};
use tablecraft_sql::parse_query;
use zip::CompressionMethod;

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
fn metadata_names_and_inline_types_produce_exact_stored_rows() {
    let package = fixture(DATA, METADATA, OBJECTS, LINKS);
    let objects = package.objects(&mut budget(), &|| false).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].name(), "People");
    assert_eq!(objects[0].data_part(), Some("data.xml"));
    let table = package
        .read_table("people", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(table.columns()[1].name(), "Full Name");
    assert_eq!(
        table.columns()[1].type_descriptor().max_text_characters(),
        Some(20)
    );
    assert_eq!(
        table.rows(),
        [
            vec![
                Value::Int32(1),
                Value::Text(" Jo&🦀 ".into()),
                Value::Boolean(true),
                Value::Decimal(Decimal::new(12500, 4).unwrap()),
                Value::DateTime(LegacyDateTime::from_ole_days(45658.5).unwrap()),
                Value::Binary(vec![1, 2])
            ],
            vec![
                Value::Int32(2),
                Value::Null,
                Value::Boolean(false),
                Value::Decimal(Decimal::new(-25, 2).unwrap()),
                Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
                Value::Null
            ],
        ]
    );
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
fn rejects_invalid_schema_names_types_values_nil_order_and_facets() {
    for invalid in [
        DATA.replace("type=\"x:int\"", "type=\"x:unknown\""),
        DATA.replace("name=\"ID\"", "name=\"id\""),
        DATA.replace("<ID>1</ID>", "<ID>2147483648</ID>"),
        DATA.replace("<ID>1</ID>", "<ID>1.5</ID>"),
        DATA.replace("<ID>1</ID>", "<ID/>"),
        DATA.replace("<Flag>true</Flag>", "<Flag>TRUE</Flag>"),
        DATA.replace("<Price>1.2500</Price>", "<Price>1e2</Price>"),
        DATA.replace("<Payload>AQI=</Payload>", "<Payload>bad!</Payload>"),
        DATA.replace(
            "<Stamp>2025-01-01T12:00:00</Stamp>",
            "<Stamp>2025-01-01T12:00:00Z</Stamp>",
        ),
        DATA.replace("maxLength value=\"20\"", "maxLength value=\"2\""),
        DATA.replace(
            "<Payload xsi:nil=\"true\"/>",
            "<Payload xsi:nil=\"true\">data</Payload>",
        ),
        DATA.replace("nillable=\"true\"", "nillable=\"false\""),
        DATA.replace("<ID>1</ID>", "<Unknown>1</Unknown>"),
        DATA.replace("<Flag>true</Flag>", "<Flag>true</Flag><ID>3</ID>"),
    ] {
        assert!(
            fixture(&invalid, METADATA, OBJECTS, LINKS)
                .read_table("People", &mut budget(), &|| false)
                .is_err(),
            "{invalid}"
        );
    }
    assert_eq!(names::decode("_x005F_x0020_").unwrap(), "_x0020_");
    assert_eq!(names::decode("_xD83E__xDD80_").unwrap(), "🦀");
    assert!(names::decode("_xD83E_").is_err());
}

#[test]
fn linked_missing_ambiguous_and_malformed_metadata_remain_explicit() {
    for metadata in [
        METADATA.replace("<m:Type>Table</m:Type>", "<m:Type>SQLLink</m:Type>"),
        METADATA.replace("<m:Name>People</m:Name>", "<m:Name/>"),
        METADATA.replace("<m:Type>Table</m:Type>", "<m:Type>Unknown</m:Type>"),
        METADATA.replace(
            "<m:Name>People</m:Name>",
            "<m:Name>People</m:Name><m:Name>Other</m:Name>",
        ),
        format!("<!DOCTYPE x [<!ENTITY y SYSTEM 'file:///secret'>]>{METADATA}"),
    ] {
        assert!(
            fixture(DATA, &metadata, OBJECTS, LINKS)
                .read_table("People", &mut budget(), &|| false)
                .is_err()
        );
    }
    let unknown = METADATA.replace("<m:Type>Table</m:Type>", "<m:Type>FutureObject</m:Type>");
    let package = fixture(DATA, &unknown, OBJECTS, LINKS);
    let objects = package.objects(&mut budget(), &|| false).unwrap();
    assert_eq!(objects[0].kind(), ObjectKind::Unknown);
    assert_eq!(objects[0].declared_type(), "FutureObject");
    assert_eq!(objects[0].name(), "People");
    let links = LINKS.replace("<Relationship Id=\"data\" Type=\"http://schemas.microsoft.com/office/access/2005/04/template/table-data\" Target=\"data.xml\"/>", "");
    let package = fixture(DATA, METADATA, OBJECTS, &links);
    assert!(
        package.objects(&mut budget(), &|| false).unwrap()[0]
            .data_part()
            .is_none()
    );
    assert!(
        package
            .read_table("People", &mut budget(), &|| false)
            .is_err()
    );
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
            Err(TemplateError::Read(InspectError::Cancelled))
        ));
    }
    let mut bytes = ResourceBudget::new(16, 100, 1000000);
    assert!(matches!(
        package.read_table("People", &mut bytes, &|| false),
        Err(TemplateError::Read(InspectError::Limit(_)))
    ));
    let mut rows = ResourceBudget::new(64 * 1024 * 1024, 1, 1000000);
    assert!(matches!(
        package.read_table("People", &mut rows, &|| false),
        Err(TemplateError::Read(InspectError::Limit(_)))
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
fn exact_decimal_facets_and_unsigned_negative_zero_follow_value_domains() {
    let restriction = r#"<x:element name="Price"><x:simpleType><x:restriction base="x:decimal"><x:totalDigits value="2"/><x:fractionDigits value="1"/></x:restriction></x:simpleType></x:element>"#;
    let source = DATA.replace(r#"<x:element name="Price" type="x:decimal"/>"#, restriction);
    for (spelling, valid) in [
        ("1.2000", true),
        ("12.0", true),
        ("1.21", false),
        ("123.0", false),
    ] {
        let data = source.replace("1.2500", spelling).replace("-0.25", "0.0");
        assert_eq!(
            fixture(&data, METADATA, OBJECTS, LINKS)
                .read_table("People", &mut budget(), &|| false)
                .is_ok(),
            valid
        );
    }
    let unsigned = DATA
        .replace("type=\"x:int\"", "type=\"x:unsignedByte\"")
        .replace("<ID>1</ID>", "<ID>-0</ID>");
    let table = fixture(&unsigned, METADATA, OBJECTS, LINKS)
        .read_table("People", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(table.rows()[0][0], Value::Byte(0));
    let list = OBJECTS.replace(
        "http://schemas.microsoft.com/office/access/2005/04/template/object",
        "http://schemas.microsoft.com/office/2007/relationships/ListInstanceDefinition",
    );
    let package = fixture(DATA, METADATA, &list, LINKS);
    assert!(package.objects(&mut budget(), &|| false).unwrap()[0].is_list_definition());
    assert!(matches!(
        package.read_table("People", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
    // A declared List Definition cannot bypass schema-agreement qualification
    // just because a well-formed inline data part happens to be present.
    assert!(matches!(
        package.table_columns("People", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
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
