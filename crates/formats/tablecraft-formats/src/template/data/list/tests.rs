use super::super::super::tests::{open, package};
use super::*;
use crate::ReadError;
use std::cell::Cell;
use tablecraft_core::values::Value;
use zip::CompressionMethod;

const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/template.xml" ContentType="text/xml"/><Override PartName="/properties.xml" ContentType="application/vnd-ms-access.objectproperties+xml"/></Types>"#;
const ROOT: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="start" Type="http://schemas.microsoft.com/office/access/2005/04/template/start" Target="template.xml"/></Relationships>"#;
const OBJECTS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="table" Type="http://schemas.microsoft.com/office/2007/relationships/ListInstanceDefinition" Target="definition.xml"/></Relationships>"#;
const LINKS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="metadata" Type="http://schemas.microsoft.com/office/access/2005/04/template/object-metadata" Target="metadata.xml"/><Relationship Id="properties" Type="http://schemas.microsoft.com/office/2007/relationships/ObjectProperties" Target="properties.xml"/><Relationship Id="data" Type="http://schemas.microsoft.com/office/access/2005/04/template/table-data" Target="data.xml"/></Relationships>"#;
const METADATA: &str = r#"<AccessObject xmlns="http://schemas.microsoft.com/office/access/2005/04/template/object-metadata"><Type>Table</Type><Name>Records</Name></AccessObject>"#;
const DATA: &str = r#"<root xmlns:x="http://www.w3.org/2001/XMLSchema"><x:schema>
<x:element name="dataroot"><x:complexType><x:sequence><x:element ref="Records" minOccurs="0" maxOccurs="unbounded"/></x:sequence></x:complexType></x:element>
<x:element name="Records"><x:complexType><x:sequence>
<x:element name="ID" type="x:int"/>
<x:element name="Label" minOccurs="0"><x:simpleType><x:restriction base="x:string"><x:maxLength value="8"/></x:restriction></x:simpleType></x:element>
<x:element name="Flag" type="x:boolean"/>
<x:element name="Cash" type="x:decimal"/>
<x:element name="Stamp" type="x:dateTime"/>
</x:sequence></x:complexType></x:element></x:schema>
<dataroot><Records><ID>1</ID><Label>Alpha</Label><Flag>true</Flag><Cash>1.25</Cash><Stamp>2025-01-01T12:00:00</Stamp></Records>
<Records><ID>2</ID><Flag>false</Flag><Cash>-0.25</Cash><Stamp>1899-12-29T06:00:00</Stamp></Records></dataroot></root>"#;
const DEFINITION: &str = r#"<Root>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><AddList xmlns="http://schemas.microsoft.com/sharepoint/soap/"><listName>Records</listName><description>Original inert metadata</description><templateID>100</templateID></AddList></s:Body></s:Envelope>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><UpdateList xmlns="http://schemas.microsoft.com/sharepoint/soap/"><listName>{12345678-1234-4234-8234-123456789abc}</listName><listProperties><List EnableAttachments="FALSE"><Validation Message="inert">[ID]&gt;0</Validation></List></listProperties>
<newFields><Fields>
<Method ID="1"><Field ID="abcdef01-2345-4678-8abc-def012345678" Name="RawLabel" DisplayName="Caption" Type="Text" MaxLength="12" Required="FALSE"><Default>Never evaluate me</Default></Field></Method>
<Method ID="2"><Field Name="Flag" Type="Boolean" Required="TRUE"/></Method>
<Method ID="3"><Field Name="Cash" Type="Currency" Required="TRUE"/></Method>
<Method ID="4"><Field Name="Stamp" Type="DateTime" Required="TRUE"/></Method>
<Method ID="5"><Field Name="Temporary" Type="Text"/></Method>
</Fields></newFields><updateFields><Fields><Method ID="6"><Field Name="ID" Type="Counter" Required="TRUE" ReadOnly="TRUE"/></Method></Fields></updateFields><deleteFields/><listVersion>0</listVersion></UpdateList></s:Body></s:Envelope>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><UpdateList xmlns="http://schemas.microsoft.com/sharepoint/soap/"><listName>Records</listName><listProperties/><newFields/><updateFields><Fields><Method ID="1"><Field ID="{ABCDEF01-2345-4678-8ABC-DEF012345678}" DisplayName="Label" MaxLength="8"/></Method></Fields></updateFields><deleteFields><Fields><Method ID="2"><Field Name="Temporary"/></Method></Fields></deleteFields><listVersion>1</listVersion></UpdateList></s:Body></s:Envelope>
</Root>"#;

fn fixture(definition: &str, data: &str, links: &str) -> TemplatePackage {
    open(&package(
        &[
            ("[Content_Types].xml", TYPES),
            ("_rels/.rels", ROOT),
            ("template.xml", "<Template/>"),
            ("_rels/template.xml.rels", OBJECTS),
            ("definition.xml", definition),
            ("_rels/definition.xml.rels", links),
            ("metadata.xml", METADATA),
            ("properties.xml", "<opaque-properties/>"),
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
fn invalid_shapes_defaults_extras_and_unsupported_calls_remain_refused() {
    for definition in [
        DEFINITION.replace("<templateID>100</templateID>", "<templateID>unknown</templateID>"),
        DEFINITION.replace("<s:Body><AddList", "<s:Header/><s:Body><AddList"),
        DEFINITION.replace("<listProperties/>", ""),
        DEFINITION.replace("<newFields/>", "<newFields/><newFields/>"),
        DEFINITION.replace("<newFields/>", "<updateFields/><newFields/>"),
        DEFINITION.replace("<Field Name=\"Flag\" Type=\"Boolean\" Required=\"TRUE\"/>", ""),
        DEFINITION.replace("<Method ID=\"6\"><Field Name=\"ID\" Type=\"Counter\" Required=\"TRUE\" ReadOnly=\"TRUE\"/></Method>", ""),
        DEFINITION.replace("Type=\"Currency\"", "Type=\"Lookup\""),
        DEFINITION.replace("<Field Name=\"Temporary\"/>", "<Field Name=\"Temporary\"/><Field Name=\"Other\"/>"),
        DEFINITION.replace("</Fields></newFields>", "<Method ID=\"99\"><Field Name=\"Extra\" Type=\"Text\" Hidden=\"TRUE\"/></Method></Fields></newFields>"),
        format!("<!DOCTYPE x [<!ENTITY value SYSTEM 'https://example.invalid/secret'>]>{DEFINITION}"),
        DEFINITION.replace("<UpdateList", "<DeleteList").replace("</UpdateList>", "</DeleteList>"),
    ] {
        assert!(fixture(&definition, DATA, LINKS).table_columns("Records", &mut budget(), &|| false).is_err(), "{definition}");
    }
    let links = LINKS.replace(
        "Type=\"http://schemas.microsoft.com/office/2007/relationships/ObjectProperties\"",
        "Type=\"urn:unknown\"",
    );
    assert!(matches!(
        fixture(DEFINITION, DATA, &links).read_table("Records", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn final_constraint_update_agrees_with_independent_last_value_model(
        lengths in proptest::collection::vec(1usize..65, 1..16),
    ) {
        let mut updates = String::new();
        for length in &lengths {
            updates.push_str(&format!(r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><UpdateList xmlns="http://schemas.microsoft.com/sharepoint/soap/"><listProperties/><newFields/><updateFields><Fields><Method ID="last"><Field Name="RawLabel" MaxLength="{length}"/></Method></Fields></updateFields></UpdateList></s:Body></s:Envelope>"#));
        }
        let definition = DEFINITION.replace("</Root>", &format!("{updates}</Root>"));
        let maximum = *lengths.last().unwrap();
        let data = DATA.replace("value=\"8\"", &format!("value=\"{maximum}\""));
        let package = fixture(&definition, &data, LINKS);
        let columns = package.table_columns("Records", &mut budget(), &|| false).unwrap();
        proptest::prop_assert_eq!(columns[1].type_descriptor().max_text_characters(), Some(maximum));
        proptest::prop_assert_eq!(package.read_table("Records", &mut budget(), &|| false).is_ok(), maximum >= "Alpha".chars().count());
    }
}

#[test]
fn named_restrictions_still_require_exact_list_definition_type_and_facet_agreement() {
    let inline = r#"<x:element name="Label" minOccurs="0"><x:simpleType><x:restriction base="x:string"><x:maxLength value="8"/></x:restriction></x:simpleType></x:element>"#;
    let field = r#"<x:element name="Label" minOccurs="0" type="StoredLabel"/>"#;
    for length in [7, 8] {
        let declaration = format!(
            r#"<x:simpleType name="StoredLabel"><x:restriction base="x:string"><x:maxLength value="{length}"/></x:restriction></x:simpleType>"#
        );
        let data = DATA
            .replace("<x:schema>", &format!("<x:schema>{declaration}"))
            .replace(inline, field);
        let package = fixture(DEFINITION, &data, LINKS);
        assert_eq!(
            package
                .table_columns("Records", &mut budget(), &|| false)
                .is_ok(),
            length == 8
        );
        assert_eq!(
            package
                .read_table("Records", &mut budget(), &|| false)
                .is_ok(),
            length == 8
        );
    }
}

mod choices;
