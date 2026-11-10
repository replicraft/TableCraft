//! Real current-image controls keep independent MR2/catalog/LVAL framing intact.
use super::*;
use crate::{inspect_table_properties, table::mutation::saved_field::Observed};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    put16, put32,
    saved_field_rename::fitting::{self, SourceCase},
    wide_properties::{block, name, names, text},
};

const SCOPE: CatalogScope = CatalogScope::new(996);
const PAGE: usize = 4096;
const CATALOG: usize = 4;
const PROPERTY: usize = 10;
const DIRECTORY: usize = 14;
const COUNT: usize = 12;
const FREE: usize = 2;
const PHYSICAL_CATALOG_SLOT: usize = 8;
const FOOTER_BYTES: usize = 5;
const PROPERTY_START_FROM_FOOTER: usize = 18;
const SINGLE: u32 = 0x4000_0000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn descriptor(bytes: &[u8]) -> usize {
    let catalog = CATALOG * PAGE;
    let start = word(bytes, catalog + DIRECTORY + PHYSICAL_CATALOG_SLOT * 2) & 0x1fff;
    let end = word(bytes, catalog + DIRECTORY + (PHYSICAL_CATALOG_SLOT - 1) * 2) & 0x1fff;
    let footer = catalog + end - FOOTER_BYTES;
    catalog + start + word(bytes, footer - PROPERTY_START_FROM_FOOTER)
}

/// Replace only the current property row, keeping its end and all retired rows.
/// The database OFF row moves with its exact payload; no production encoder is used.
fn replace_live_properties(before: &[u8], properties: &[u8]) -> Vec<u8> {
    let page = PROPERTY * PAGE;
    let count = word(before, page + COUNT);
    assert_eq!(count, 3);
    let slot = count - 2;
    let end = word(before, page + DIRECTORY + (slot - 1) * 2);
    let old_start = word(before, page + DIRECTORY + slot * 2);
    let db_start = word(before, page + DIRECTORY + (slot + 1) * 2);
    let new_start = end - properties.len();
    let new_database = new_start - (old_start - db_start);
    assert!(new_database >= DIRECTORY + count * 2);
    let mut after = before.to_vec();
    after[page + new_database..page + new_start]
        .copy_from_slice(&before[page + db_start..page + old_start]);
    after[page + new_start..page + end].copy_from_slice(properties);
    put16(&mut after, page + DIRECTORY + slot * 2, new_start as u16);
    put16(
        &mut after,
        page + DIRECTORY + (slot + 1) * 2,
        new_database as u16,
    );
    put16(
        &mut after,
        page + FREE,
        (new_database - DIRECTORY - count * 2) as u16,
    );
    put32(
        &mut after,
        descriptor(before),
        SINGLE | properties.len() as u32,
    );
    after
}

fn property(index: u16, flags: u8, kind: u8, raw: &[u8]) -> Vec<u8> {
    let mut bytes = ((raw.len() + 8) as u16).to_le_bytes().to_vec();
    bytes.extend([flags, kind]);
    bytes.extend(index.to_le_bytes());
    bytes.extend((raw.len() as u16).to_le_bytes());
    bytes.extend(raw);
    bytes
}

fn group(label: &str, entries: &[Vec<u8>]) -> Vec<u8> {
    let label = name(label);
    let mut body = ((label.len() + 4) as u32).to_le_bytes().to_vec();
    body.extend(label);
    for entry in entries {
        body.extend(entry);
    }
    block(if body[4..6] == [0, 0] { 0 } else { 1 }, &body)
}

fn bag(control: &str) -> Vec<u8> {
    let mut keys = vec!["Required", "DefaultValue"];
    if matches!(control, "ValidationRule" | "NameMap" | "Expression") {
        keys.push(control);
    }
    let mut raw = b"MR2\0".to_vec();
    raw.extend(names(&keys));
    raw.extend(group(
        "Score",
        &[property(0, 1, 1, &[0]), property(1, 1, 12, &text("0"))],
    ));
    let default = if control == "referencing default" {
        "[Score]"
    } else {
        "0"
    };
    let mut sibling = vec![property(0, 1, 1, &[0]), property(1, 1, 12, &text(default))];
    if control == "Expression" {
        sibling.push(property(2, 1, 12, &text("[Score]")));
    }
    raw.extend(group("Score2", &sibling));
    if control == "ValidationRule" {
        raw.extend(group("", &[property(2, 1, 12, &text("[Score] >= 0"))]));
    } else if control == "NameMap" {
        raw.extend(group("", &[property(2, 0, 11, &[1, 2, 3])]));
    }
    raw
}

#[test]
fn current_source_refuses_live_name_dependencies_even_with_explicit_tracking_off() {
    let original = fitting::source(SourceCase::NumberHistory);
    let baseline = replace_live_properties(&original, &bag("baseline"));
    observe(&image(&baseline), "Table1", "Score", &mut budget(), &|| {
        false
    })
    .unwrap();

    for control in [
        "referencing default",
        "ValidationRule",
        "NameMap",
        "Expression",
    ] {
        let bytes = replace_live_properties(&original, &bag(control));
        let native = image(&bytes);
        let properties = inspect_table_properties(
            &mut native.reader(),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(properties.raw_bytes(), bag(control));
        let observed =
            Observed::capture(&native, "Table1", "Score", &mut budget(), &|| false).unwrap();
        let refusal = dependencies(&properties, &observed, &mut budget(), &|| false);
        if control == "referencing default" {
            assert!(
                matches!(
                    refusal,
                    Err(Error::RowSource(crate::EditError::Unsupported(_)))
                ),
                "dependency {control}"
            );
        } else {
            assert!(
                matches!(refusal, Err(Error::Source)),
                "dependency {control}"
            );
        }
        assert!(
            observe(&native, "Table1", "Score", &mut budget(), &|| false).is_err(),
            "full source {control}"
        );
        assert_eq!(native.as_bytes(), bytes);
    }
}

#[test]
fn current_source_rejects_stale_lengths_shared_live_rows_and_tracking_enabled() {
    let original = fitting::source(SourceCase::NumberHistory);
    for control in 0..4 {
        let mut bytes = original.clone();
        match control {
            0 => {
                let at = descriptor(&bytes);
                let tagged = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                put32(&mut bytes, at, tagged - 2);
            }
            1 => {
                let at = descriptor(&bytes);
                put32(&mut bytes, at + 4, (PROPERTY as u32) << 8 | 2);
            }
            2 => {
                let page = PROPERTY * PAGE;
                let start = word(&bytes, page + DIRECTORY + 4);
                let end = word(&bytes, page + DIRECTORY + 2);
                let needle = text("Track Name AutoCorrect Info");
                assert!(
                    bytes[page + start..page + end]
                        .windows(needle.len())
                        .any(|part| part == needle)
                );
                // The final two INT32 property values are literal zero in the
                // independent OFF bag. Change its final value to true.
                put32(&mut bytes, page + end - 4, 1);
            }
            3 => {
                let catalog = CATALOG * PAGE;
                let start = word(&bytes, catalog + DIRECTORY + 3 * 2);
                let end = word(&bytes, catalog + DIRECTORY + 2 * 2);
                let expected = text("Relationships");
                assert!(
                    bytes[catalog + start..catalog + end]
                        .windows(expected.len())
                        .any(|part| part == expected)
                );
                assert_eq!(word(&bytes, catalog + start + 10), 3);
                put16(&mut bytes, catalog + start + 10, 5);
            }
            _ => unreachable!(),
        }
        let native = image(&bytes);
        assert!(
            observe(&native, "Table1", "Score", &mut budget(), &|| false).is_err(),
            "control {control}"
        );
        assert_eq!(native.as_bytes(), bytes);
    }
}
