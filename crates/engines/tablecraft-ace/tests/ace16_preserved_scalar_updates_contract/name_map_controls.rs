//! Separately authored readable NameMap anomalies, never writable authority.
use super::{SCOPE, budget, model, name_map};
use std::io::Cursor;
use tablecraft_ace::{PropertyGroupKind, inspect_table_properties, inspect_table_schema};

#[derive(Clone, Copy)]
enum Anomaly {
    MissingAttachmentAliasesField,
    MissingAttachmentAliasesTable,
    ForeignParent,
    UnknownKind,
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn source(anomaly: Anomaly) -> model::Case {
    let mut case = name_map::source(true, false);
    for bytes in [&mut case.before, &mut case.expected] {
        let starts = bytes
            .windows(4)
            .enumerate()
            .filter_map(|(at, value)| (value == [0x0a, 0xcc, 0x0e, 0x55]).then_some(at))
            .collect::<Vec<_>>();
        assert_eq!(starts.len(), 1);
        let base = starts[0];
        let properties = inspect_table_properties(
            &mut Cursor::new(&*bytes),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let map = properties
            .group(PropertyGroupKind::Table, "")
            .unwrap()
            .get("NameMap")
            .unwrap();
        assert_eq!(&bytes[base..base + map.raw_value().len()], map.raw_value());
        let mut record = base + 8;
        // Table then sixteen column records. This source traversal owns literal
        // framing only; it never calls the preservation validator.
        for ordinal in 0..=16 {
            assert_eq!(long(bytes, record + 16), if ordinal == 0 { 0 } else { 7 });
            if ordinal == 15 {
                match anomaly {
                    Anomaly::MissingAttachmentAliasesField => bytes[record..record + 16].fill(4),
                    Anomaly::MissingAttachmentAliasesTable => bytes[record..record + 16].fill(0x91),
                    Anomaly::ForeignParent => bytes[record + 20..record + 36].fill(0x92),
                    Anomaly::UnknownKind => {
                        bytes[record + 16..record + 20].copy_from_slice(&99_u32.to_le_bytes())
                    }
                }
                break;
            }
            record += 36;
            while bytes[record..record + 2] != [0, 0] {
                record += 2;
            }
            record += 6;
        }
    }
    case
}

pub(super) fn cases() -> impl Iterator<Item = model::Case> {
    [
        Anomaly::MissingAttachmentAliasesField,
        Anomaly::MissingAttachmentAliasesTable,
        Anomaly::ForeignParent,
        Anomaly::UnknownKind,
    ]
    .into_iter()
    .map(source)
}

#[test]
fn qualify_preserved_name_map_anomaly_sources_before_validator() {
    for case in cases() {
        assert_eq!(
            case.before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(
                    |(at, (before, after))| (before != after).then_some((at, before ^ after))
                )
                .collect::<Vec<_>>(),
            [(model::BOOLEAN_PATCH_OFFSET, 4)]
        );
        for bytes in [&case.before, &case.expected] {
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(schema.columns().len(), 16);
            let properties = inspect_table_properties(
                &mut Cursor::new(bytes),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                properties
                    .group(PropertyGroupKind::Column, "Attachment_Field")
                    .unwrap()
                    .get("GUID")
                    .is_none()
            );
            let map = properties
                .group(PropertyGroupKind::Table, "")
                .unwrap()
                .get("NameMap")
                .unwrap();
            assert_eq!(&map.raw_value()[..4], &[0x0a, 0xcc, 0x0e, 0x55]);
        }
    }
}
