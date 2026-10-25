use super::*;
use std::io::{Cursor, Write};
use zip::{ZipWriter, write::SimpleFileOptions};

fn encoded(text: &str, mode: u8) -> Vec<u8> {
    let label = if mode < 2 { "UTF-8" } else { "UTF-16" };
    let text = format!("<?xml version=\"1.0\" encoding=\"{label}\"?>{text}");
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

fn package_bytes(mode: u8, data: &[u8], method: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in [
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", ROOT),
        ("template.xml", "<inert/>"),
        ("_rels/template.xml.rels", OBJECTS),
        ("definition.xml", "opaque inert definition"),
        ("_rels/definition.xml.rels", LINKS),
        ("metadata.xml", METADATA),
        ("data.xml", ""),
    ] {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        if name == "data.xml" {
            writer.write_all(data).unwrap();
        } else if name == "definition.xml" {
            writer.write_all(text.as_bytes()).unwrap();
        } else {
            writer.write_all(&encoded(text, mode)).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn mandatory_encodings_cover_controls_metadata_schema_and_exact_stored_table_values() {
    let expected = fixture(DATA, METADATA, OBJECTS, LINKS)
        .read_table("People", &mut budget(), &|| false)
        .unwrap();
    for mode in 0..4 {
        for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
            let raw = package_bytes(mode, &encoded(DATA, mode), method);
            let before = raw.clone();
            let package = open(&raw).unwrap();
            assert_eq!(
                package.part("data.xml").unwrap().bytes(),
                encoded(DATA, mode)
            );
            assert!(
                package
                    .table_columns("People", &mut budget(), &|| false)
                    .unwrap()
                    == expected.columns()
            );
            let table = package
                .read_table("People", &mut budget(), &|| false)
                .unwrap();
            assert!(table.columns() == expected.columns());
            assert_eq!(table.rows(), expected.rows());
            assert_eq!(raw, before);
        }
    }
}

#[test]
fn schema_discovery_does_not_decode_or_validate_malformed_data_suffixes() {
    let end = DATA.find("</x:schema>").unwrap() + "</x:schema>".len();
    for mode in 0..4 {
        let mut data = encoded(&DATA[..end], mode);
        // The very first source unit after schema EOF is malformed; no chunk
        // prefetch, surrogate check or data event may inspect it during binding.
        data.extend(if mode < 2 {
            vec![0xff]
        } else if mode == 2 {
            vec![0, 0xd8]
        } else {
            vec![0xd8, 0]
        });
        let package = open(&package_bytes(mode, &data, CompressionMethod::Stored)).unwrap();
        assert_eq!(
            package
                .table_columns("People", &mut budget(), &|| false)
                .unwrap()
                .len(),
            6
        );
        assert!(matches!(
            package.read_table("People", &mut budget(), &|| false),
            Err(TemplateError::Malformed)
        ));
    }
}

#[test]
fn declaration_mismatches_and_unsupported_markup_fail_at_the_same_inert_boundaries() {
    for mode in [2, 3] {
        for body in [
            format!("<!DOCTYPE root SYSTEM 'https://example.invalid/external'>{DATA}"),
            format!("<?external inert?>{DATA}"),
            DATA.replace(" Jo&amp;🦀 ", "&unknown;"),
        ] {
            let package = open(&package_bytes(
                mode,
                &encoded(&body, mode),
                CompressionMethod::Stored,
            ))
            .unwrap();
            assert!(
                package
                    .read_table("People", &mut budget(), &|| false)
                    .is_err()
            );
        }
        let bytes = encoded(DATA, mode);
        let package = package_bytes(0, &bytes, CompressionMethod::Stored);
        // Outer UTF-8 OPC controls and inner UTF-16 data are deliberately mixed.
        assert!(
            open(&package)
                .unwrap()
                .read_table("People", &mut budget(), &|| false)
                .is_ok()
        );
    }
    let wrong = encoded(DATA, 0)
        .into_iter()
        .flat_map(|byte| [byte, 0])
        .collect::<Vec<_>>();
    let package = open(&package_bytes(0, &wrong, CompressionMethod::Stored)).unwrap();
    assert!(
        package
            .table_columns("People", &mut budget(), &|| false)
            .is_err()
    );
}
