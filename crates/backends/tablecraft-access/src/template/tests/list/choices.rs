use super::*;
use std::io::{Cursor, Write};
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
