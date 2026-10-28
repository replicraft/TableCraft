use super::*;
use tablecraft_core::values::{Double, Single};

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

fn columns(data: &str) -> Result<Vec<Column>, TemplateError> {
    fixture(data, METADATA, OBJECTS, LINKS).table_columns("People", &mut budget(), &|| false)
}

#[test]
fn named_restrictions_preserve_all_existing_primitive_domains_and_exact_cells() {
    let cases = [
        ("string", "é🦀", Value::Text("é🦀".into())),
        ("boolean", "true", Value::Boolean(true)),
        ("byte", "-128", Value::Int16(-128)),
        ("unsignedByte", "255", Value::Byte(255)),
        ("short", "-32768", Value::Int16(-32768)),
        ("unsignedShort", "65535", Value::Int32(65535)),
        ("int", "-2147483648", Value::Int32(i32::MIN)),
        ("unsignedInt", "4294967295", Value::Int64(4294967295)),
        ("long", "-9223372036854775808", Value::Int64(i64::MIN)),
        ("float", "1.5", Value::Single(Single::from_bits(0x3fc00000))),
        (
            "double",
            "-2.5",
            Value::Double(Double::from_bits(0xc004000000000000)),
        ),
        (
            "decimal",
            "1.2500",
            Value::Decimal(Decimal::new(12500, 4).unwrap()),
        ),
        (
            "dateTime",
            "1899-12-30T00:00:00",
            Value::DateTime(LegacyDateTime::from_ole_days(0.0).unwrap()),
        ),
        ("base64Binary", "AQI=", Value::Binary(vec![1, 2])),
        ("hexBinary", "0102", Value::Binary(vec![1, 2])),
    ];
    let mut types = String::new();
    let mut fields = String::new();
    let mut cells = String::new();
    let mut expected = Vec::new();
    for (index, (kind, spelling, value)) in cases.into_iter().enumerate() {
        types.push_str(&named(&format!("Stored{index}"), &format!("x:{kind}"), ""));
        fields.push_str(&format!(
            r#"<x:element name="F{index}" type="Stored{index}"/>"#
        ));
        cells.push_str(&format!("<F{index}>{spelling}</F{index}>"));
        expected.push(value);
    }
    let data = document(&types, &fields, &format!("<People>{cells}</People>"));
    for mode in 0..4 {
        let bytes = super::encoding::package_bytes(
            mode,
            &super::encoding::encoded(&data, mode),
            CompressionMethod::Deflated,
        );
        let package = open(&bytes).unwrap();
        let table = package
            .read_table("People", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(table.rows(), [expected.clone()]);
        assert_eq!(
            package
                .table_columns("People", &mut budget(), &|| false)
                .unwrap()
                .len(),
            15
        );
    }
}

#[test]
fn forward_references_exact_type_names_and_shared_domains_do_not_change_namespace_rules() {
    let types = [
        named("Narrow", "Wide", r#"<x:maxLength value="4"/>"#),
        named("Wide", "x:string", r#"<x:maxLength value="8"/>"#),
        named("wide", "x:int", ""),
        named("int", "x:string", ""),
    ]
    .concat();
    let fields = r#"<x:element name="Label" type="Narrow"/><x:element name="Other" type="Narrow"/><x:element name="ID" type="wide"/><x:element name="Literal" type="int"/><x:element name="Builtin" type="x:int"/>"#;
    let rows = "<People><Label>é🦀</Label><Other>four</Other><ID>7</ID><Literal>text</Literal><Builtin>9</Builtin></People>";
    let data = document(&types, fields, rows);
    let table = read(&data).unwrap();
    assert_eq!(
        table.rows(),
        [vec![
            Value::Text("é🦀".into()),
            Value::Text("four".into()),
            Value::Int32(7),
            Value::Text("text".into()),
            Value::Int32(9)
        ]]
    );
    assert_eq!(
        table.columns()[0].type_descriptor().max_text_characters(),
        Some(4)
    );
    assert!(read(&data.replace("<Other>four</Other>", "<Other>longer</Other>")).is_err());
    assert!(columns(&data.replace("type=\"Narrow\"", "type=\"narrow\"")).is_err());
}

#[test]
fn inherited_and_inline_facets_narrow_exact_text_binary_and_decimal_domains() {
    let types = [
        named("TextBase", "x:string", r#"<x:maxLength value="8"/>"#),
        named("TextSmall", "TextBase", r#"<x:maxLength value="4"/>"#),
        named("Blob", "x:base64Binary", r#"<x:maxLength value="2"/>"#),
        named(
            "Price",
            "x:decimal",
            r#"<x:totalDigits value="5"/><x:fractionDigits value="2"/>"#,
        ),
        named("PriceSmall", "Price", r#"<x:totalDigits value="4"/>"#),
    ]
    .concat();
    let fields = r#"<x:element name="Label"><x:simpleType><x:restriction base="TextSmall"><x:maxLength value="2"/></x:restriction></x:simpleType></x:element><x:element name="Payload" type="Blob"/><x:element name="Amount"><x:simpleType><x:restriction base="PriceSmall"><x:fractionDigits value="1"/></x:restriction></x:simpleType></x:element>"#;
    let rows = "<People><Label>é🦀</Label><Payload>AQI=</Payload><Amount>1.2000</Amount></People>";
    let data = document(&types, fields, rows);
    let table = read(&data).unwrap();
    assert_eq!(
        table.columns()[0].type_descriptor().max_text_characters(),
        Some(2)
    );
    assert_eq!(
        table.rows(),
        [vec![
            Value::Text("é🦀".into()),
            Value::Binary(vec![1, 2]),
            Value::Decimal(Decimal::new(12000, 4).unwrap())
        ]]
    );
    for changed in [
        data.replace("é🦀", "abc"),
        data.replace("AQI=", "AQID"),
        data.replace("1.2000", "1.23"),
        data.replace("1.2000", "12345"),
    ] {
        assert!(read(&changed).is_err());
    }
}

#[test]
fn all_declarations_preflight_cycles_unknowns_duplicates_and_unsupported_or_widened_facets() {
    let invalid = [
        named("A", "A", ""),
        [named("A", "B", ""), named("B", "A", "")].concat(),
        named("A", "Missing", ""),
        named("A", "e:External", ""),
        [named("A", "x:int", ""), named("A", "x:string", "")].concat(),
        named("A", "x:int", r#"<x:maxLength value="1"/>"#),
        named("A", "x:string", r#"<x:pattern value=".*"/>"#),
        named("A", "x:string", r#"<x:maxLength value="2"/><x:maxLength value="2"/>"#),
        [named("A", "x:string", r#"<x:maxLength value="2"/>"#), named("B", "A", r#"<x:maxLength value="3"/>"#)].concat(),
        [named("A", "x:decimal", r#"<x:totalDigits value="3"/>"#), named("B", "A", r#"<x:totalDigits value="4"/>"#)].concat(),
        [named("A", "x:decimal", r#"<x:fractionDigits value="1"/>"#), named("B", "A", r#"<x:fractionDigits value="2"/>"#)].concat(),
        [named("A", "x:decimal", r#"<x:fractionDigits value="2"/>"#), named("B", "A", r#"<x:totalDigits value="1"/>"#)].concat(),
        r#"<x:import namespace="urn:external" schemaLocation="https://example.invalid/not-opened"/>"#.into(),
        r#"<x:simpleType name="A"><x:list itemType="x:int"/></x:simpleType>"#.into(),
    ];
    for types in invalid {
        // A is deliberately unused and the row suffix is malformed. Neither
        // empty input nor scalar pruning can hide invalid schema declarations.
        let data = document(&types, r#"<x:element name="ID" type="x:int"/>"#, "<broken");
        assert!(columns(&data).is_err(), "invalid declaration was admitted");
    }
}

#[test]
fn type_count_and_reference_depth_limits_hold_for_both_definition_orders() {
    let fields = r#"<x:element name="ID" type="T0"/>"#;
    for count in [256, 257] {
        let types = (0..count)
            .map(|index| named(&format!("T{index}"), "x:int", ""))
            .collect::<String>();
        let data = document(&types, fields, "<People><ID>1</ID></People>");
        assert_eq!(columns(&data).is_ok(), count == 256);
    }
    for count in [64, 65] {
        for reverse in [false, true] {
            let mut types = (0..count)
                .map(|index| {
                    let base = if index + 1 == count {
                        "x:int".into()
                    } else {
                        format!("T{}", index + 1)
                    };
                    named(&format!("T{index}"), &base, "")
                })
                .collect::<Vec<_>>();
            if reverse {
                types.reverse();
            }
            let data = document(&types.concat(), fields, "<People><ID>1</ID></People>");
            assert_eq!(read(&data).is_ok(), count == 64);
        }
    }
}

#[test]
fn resolution_consumes_shared_work_and_allocation() {
    let types = [named("A", "B", ""), named("B", "x:int", "")].concat();
    let data = document(
        &types,
        r#"<x:element name="ID" type="A"/>"#,
        "<People><ID>1</ID></People>",
    );
    let root = tree::parse(data.as_bytes(), &mut budget(), &|| false).unwrap();
    let node = &root.children[0];
    for mut pool in [
        ResourceBudget::new(0, 0, 100_000),
        ResourceBudget::new(100_000, 0, 0),
    ] {
        assert!(matches!(
            schema::read(node, "People", &mut pool, &|| false),
            Err(TemplateError::Read(ReadError::Limit(_)))
        ));
    }
}
