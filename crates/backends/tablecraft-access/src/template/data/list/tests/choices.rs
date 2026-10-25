use super::super::choices as option_metadata;
use super::*;
use std::io::{Cursor, Write};
use tablecraft_core::limits::Resource;
use zip::{ZipWriter, write::SimpleFileOptions};

const OPTIONS: &str = r#"<CHOICES><CHOICE>Alpha</CHOICE><CHOICE Value="$Resources:inert" JumpTo="https://example.invalid/not-opened">é&amp;🦀</CHOICE></CHOICES>"#;

fn definition(base: Option<&str>, children: &str) -> String {
    let base = base
        .map(|value| format!(r#" BaseType="{value}""#))
        .unwrap_or_default();
    DEFINITION
        .replace(r#"Type="Text" MaxLength="12""#, &format!(r#"Type="Choice"{base} MaxLength="12""#))
        .replace("<Default>Never evaluate me</Default>", &format!("{children}<Default>Never evaluate me</Default><DefaultFormula>NeverRun()</DefaultFormula>"))
}

fn encode(text: &str, mode: u8) -> Vec<u8> {
    let label = if mode < 2 { "UTF-8" } else { "UTF-16" };
    let text = format!(r#"<?xml version="1.0" encoding="{label}"?>{text}"#);
    match mode {
        0 => text.into_bytes(),
        1 => [vec![0xef, 0xbb, 0xbf], text.into_bytes()].concat(),
        _ => {
            let little = mode == 2;
            let mut bytes = if little {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                bytes.extend(if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            bytes
        }
    }
}

fn encoded_package(definition: &str, data: &str, mode: u8) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in [
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", ROOT),
        ("template.xml", "<inert/>"),
        ("_rels/template.xml.rels", OBJECTS),
        ("definition.xml", definition),
        ("_rels/definition.xml.rels", LINKS),
        ("metadata.xml", METADATA),
        ("properties.xml", "<opaque-properties/>"),
        ("data.xml", data),
    ] {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(&encode(text, mode)).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn textual_choice_storage_queries_exact_cells_in_all_mandatory_xml_encodings() {
    for base in [None, Some("Text")] {
        for mode in 0..4 {
            let definition = definition(base, OPTIONS);
            let bytes = encoded_package(&definition, DATA, mode);
            let before = bytes.clone();
            let package = open(&bytes).unwrap();
            assert_eq!(
                package.part("definition.xml").unwrap().bytes(),
                encode(&definition, mode)
            );
            let source = TemplateSource::new(&package, budget(), &|| false).unwrap();
            let parsed = parse_query("SELECT Label FROM Records", Default::default()).unwrap();
            assert_eq!(
                prepare(parsed.query(), &source)
                    .unwrap()
                    .execute(&source, &[], Default::default())
                    .unwrap()
                    .rows(),
                [vec![Value::Text("Alpha".into())], vec![Value::Null]]
            );
            assert_eq!(bytes, before);
        }
    }
}

#[test]
fn stored_strings_are_not_options_membership_or_default_display_or_navigation_results() {
    for options in [
        "",
        "<CHOICES/>",
        "<CHOICES><CHOICE>Other</CHOICE><CHOICE>Other</CHOICE><CHOICE/></CHOICES>",
        OPTIONS,
    ] {
        for fill in ["TRUE", "FALSE"] {
            let definition = definition(None, options).replace(
                r#"Type="Choice""#,
                &format!(r#"Type="Choice" FillInChoice="{fill}""#),
            );
            for (spelling, expected) in [
                ("Alpha", "Alpha"),
                ("A;#B", "A;#B"),
                ("é&amp;🦀", "é&🦀"),
                ("e\u{301}", "e\u{301}"),
                ("", ""),
            ] {
                let data = DATA.replace(
                    "<Label>Alpha</Label>",
                    &format!("<Label>{spelling}</Label>"),
                );
                let table = fixture(&definition, &data, LINKS)
                    .read_table("Records", &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(table.rows()[0][1], Value::Text(expected.into()));
                assert_eq!(table.rows()[1][1], Value::Null);
            }
        }
    }
}

#[test]
fn partial_updates_retain_storage_overrides_until_an_explicit_replacement() {
    let partial = r#"DisplayName="Label" MaxLength="8""#;
    for base in ["Integer", "Future", ""] {
        let unsupported = definition(Some(base), OPTIONS);
        assert!(matches!(
            fixture(&unsupported, DATA, LINKS).table_columns("Records", &mut budget(), &|| false),
            Err(TemplateError::Unsupported)
        ));
        let replaced = unsupported.replace(partial, &format!(r#"{partial} BaseType="Text""#));
        assert!(
            fixture(&replaced, DATA, LINKS)
                .read_table("Records", &mut budget(), &|| false)
                .is_ok()
        );
    }
    let text = definition(Some("Text"), OPTIONS);
    let changed = text.replace(partial, &format!(r#"{partial} BaseType="Integer""#));
    assert!(matches!(
        fixture(&changed, DATA, LINKS).table_columns("Records", &mut budget(), &|| false),
        Err(TemplateError::Unsupported)
    ));
}

#[test]
fn unsupported_choice_shapes_and_nontext_carriers_refuse_before_data_events() {
    for children in [
        "<CHOICES/><CHOICES/>",
        "<choices/>",
        r#"<CHOICES extra="1"/>"#,
        "<CHOICES><choice>Alpha</choice></CHOICES>",
        "<CHOICES><CHOICE><Nested/></CHOICE></CHOICES>",
        r#"<CHOICES><CHOICE unknown="1">Alpha</CHOICE></CHOICES>"#,
        r#"<CHOICES><CHOICE xmlns:o="urn:schemas-microsoft-com:officedata" o:Value="1">Alpha</CHOICE></CHOICES>"#,
        r#"<CHOICES xmlns="urn:unknown"/>"#,
    ] {
        let definition = definition(None, children);
        let bad_rows = DATA.replace("<Label>Alpha</Label>", "<broken");
        assert!(
            fixture(&definition, &bad_rows, LINKS)
                .table_columns("Records", &mut budget(), &|| false)
                .is_err()
        );
    }
    for kind in [
        "MultiChoice",
        "GridChoice",
        "Lookup",
        "LookupMulti",
        "Calculated",
    ] {
        let definition =
            definition(None, OPTIONS).replace(r#"Type="Choice""#, &format!(r#"Type="{kind}""#));
        assert!(
            fixture(&definition, DATA, LINKS)
                .table_columns("Records", &mut budget(), &|| false)
                .is_err()
        );
    }
    let definition = definition(None, OPTIONS);
    let inline = r#"<x:element name="Label" minOccurs="0"><x:simpleType><x:restriction base="x:string"><x:maxLength value="8"/></x:restriction></x:simpleType></x:element>"#;
    let numeric = DATA.replace(
        inline,
        r#"<x:element name="Label" minOccurs="0" type="x:int"/>"#,
    );
    assert!(matches!(
        fixture(&definition, &numeric, LINKS).table_columns("Records", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
    let bad_boolean =
        definition.replace(r#"Type="Choice""#, r#"Type="Choice" FillInChoice="true""#);
    assert!(matches!(
        fixture(&bad_boolean, DATA, LINKS).table_columns("Records", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
}

#[test]
fn known_choices_on_other_scalar_fields_remain_inert_as_the_field_contract_requires() {
    let definition = DEFINITION.replace("<Default>Never evaluate me</Default>", "<CHOICES arbitrary=\"inert\"><Unknown>NeverRun()</Unknown></CHOICES><Default>Never evaluate me</Default>");
    let table = fixture(&definition, DATA, LINKS)
        .read_table("Records", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(table.rows()[0][1], Value::Text("Alpha".into()));
    assert_eq!(table.rows()[1][1], Value::Null);
}

#[test]
fn named_string_restrictions_and_required_maximum_agreement_still_apply_to_choices() {
    let definition = definition(Some("Text"), OPTIONS);
    let inline = r#"<x:element name="Label" minOccurs="0"><x:simpleType><x:restriction base="x:string"><x:maxLength value="8"/></x:restriction></x:simpleType></x:element>"#;
    let named = r#"<x:simpleType name="StoredChoice"><x:restriction base="x:string"><x:maxLength value="8"/></x:restriction></x:simpleType>"#;
    let data = DATA
        .replace("<x:schema>", &format!("<x:schema>{named}"))
        .replace(
            inline,
            r#"<x:element name="Label" minOccurs="0" type="StoredChoice"/>"#,
        );
    assert!(
        fixture(&definition, &data, LINKS)
            .read_table("Records", &mut budget(), &|| false)
            .is_ok()
    );
    let wrong_maximum = data.replace(r#"value="8""#, r#"value="7""#);
    assert!(matches!(
        fixture(&definition, &wrong_maximum, LINKS).table_columns(
            "Records",
            &mut budget(),
            &|| false
        ),
        Err(TemplateError::Malformed)
    ));
    let required = definition.replace(r#"Required="FALSE""#, r#"Required="TRUE""#);
    assert!(matches!(
        fixture(&required, &data, LINKS).table_columns("Records", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
}

#[test]
fn option_validation_checks_cumulative_work_and_every_observed_cancellation_point() {
    let options = (0..32)
        .map(|index| format!("<CHOICE Value=\"display{index}\">Option{index}</CHOICE>"))
        .collect::<String>();
    let xml = format!("<CHOICES>{options}</CHOICES>");
    let node = tree::parse(xml.as_bytes(), &mut budget(), &|| false).unwrap();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    option_metadata::validate(&node, &mut budget(), &count).unwrap();
    for stop in 1..=calls.get() {
        let current = Cell::new(0);
        let cancelled = || {
            current.set(current.get() + 1);
            current.get() == stop
        };
        assert!(matches!(
            option_metadata::validate(&node, &mut budget(), &cancelled),
            Err(TemplateError::Read(InspectError::Cancelled))
        ));
    }
    let mut limited = ResourceBudget::new(0, 0, 1);
    assert!(
        matches!(option_metadata::validate(&node, &mut limited, &|| false), Err(TemplateError::Read(InspectError::Limit(limit))) if limit.resource == Resource::Steps)
    );
}
