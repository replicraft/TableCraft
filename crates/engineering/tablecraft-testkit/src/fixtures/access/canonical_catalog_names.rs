//! Independent SOURCEs separating stored catalog Text from raw definition names.
use super::{
    canonical_creation, canonical_native17_creation, data_page, database_properties, put16, put32,
};

/// One immutable name-encoding SOURCE recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// A table whose entire name is one U+FEFF.
    TableBom,
    /// Leading U+FEFF plus Latin/Greek/CJK/non-BMP/ASCII units.
    TableMixed,
    /// The 64-UTF-16-unit table-name boundary, all U+FEFF.
    TableBoundary,
    /// A same-width raw definition-name control with U+FEFF and U+0100.
    ColumnRaw,
}

/// Original qualification order, without deriving names from producer output.
pub const CASES: [Case; 4] = [
    Case::TableBom,
    Case::TableMixed,
    Case::TableBoundary,
    Case::ColumnRaw,
];

impl Case {
    /// Stable file/log identity independent of its Unicode name.
    pub const fn id(self) -> &'static str {
        match self {
            Self::TableBom => "table-bom",
            Self::TableMixed => "table-mixed",
            Self::TableBoundary => "table-boundary",
            Self::ColumnRaw => "column-raw",
        }
    }
}

/// Complete original and literal metadata expected from independent readers.
pub struct Source {
    /// Exact decoded user-table name, without normalization.
    pub table: String,
    /// Exact raw-definition column names; both remain ordinary Int32.
    pub columns: [String; 2],
    /// Literal stored catalog Name payload, not a production-encoded value.
    pub catalog_name_payload: Vec<u8>,
    /// Complete nine-page SOURCE with no user rows.
    pub bytes: Vec<u8>,
}

/// Build one closed original from the unchanged qualified canonical envelope.
/// Every catalog DATA record is independently reserialized from literal facts.
pub fn source(case: Case) -> Source {
    let (table, payload) = match case {
        Case::TableBom => ("\u{feff}".into(), vec![0xff, 0xfe, 0, 0xff, 0xfe]),
        Case::TableMixed => (
            "\u{feff}āΩ雪🙂A".into(),
            vec![
                0xff, 0xfe, 0, 0xff, 0xfe, 1, 1, 0xa9, 3, 0xea, 0x96, 0x3d, 0xd8, 0x42, 0xde, 0x41,
                0,
            ],
        ),
        Case::TableBoundary => {
            let mut payload = vec![0xff, 0xfe, 0];
            for _ in 0..64 {
                payload.extend_from_slice(&[0xff, 0xfe]);
            }
            ("\u{feff}".repeat(64), payload)
        }
        Case::ColumnRaw => ("Items".into(), b"I\0t\0e\0m\0s\0".to_vec()),
    };
    let mut bytes = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog_page(&payload));

    let score = if case == Case::ColumnRaw {
        let start = 7 * 4096 + 121;
        assert_eq!(&bytes[start..start + 10], b"S\0c\0o\0r\0e\0");
        bytes[start..start + 10].copy_from_slice(&[0xff, 0xfe, 0, 1, b'a', 0, b'b', 0, b'c', 0]);
        "\u{feff}Āabc"
    } else {
        "Score"
    };
    Source {
        table,
        columns: ["Id".into(), score.into()],
        catalog_name_payload: payload,
        bytes,
    }
}

fn catalog_page(payload: &[u8]) -> Vec<u8> {
    let objects = [
        (0x0f00_0000, 0, "DatabaseRoot", 3, 0x8000_0002),
        (0x0f00_0001, 0x0f00_0000, "Tables", 3, 0x8000_0002),
        (0x0f00_0002, 0x0f00_0000, "Databases", 3, 0x8000_0002),
        (0x0f00_0003, 0x0f00_0000, "Relationships", 3, 0x8000_0002),
        (2, 0x0f00_0001, "MSysObjects", 1, 0x8000_0002),
        (5, 0x0f00_0001, "MSysRelationships", 1, 0x8000_0002),
        (6, 0x0f00_0001, "MSysACEs", 1, 0x8000_0002),
    ];
    let mut records = Vec::new();
    for (id, parent, name, kind, flags) in objects {
        let raw = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        records.push((0, catalog_record(id, parent, kind, flags, &raw)));
    }
    records.push((0, catalog_record(7, 0x0f00_0001, 1, 0, payload)));
    let payload_bytes = records.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut catalog = data_page(2, &records);
    put16(&mut catalog, 2, (4096 - 14 - 2 * 8 - payload_bytes) as u16);
    catalog
}

fn catalog_record(id: u32, parent: u32, kind: u16, flags: u32, name: &[u8]) -> Vec<u8> {
    let mut record = vec![0; 32];
    put16(&mut record, 0, 15);
    put32(&mut record, 2, id);
    put32(&mut record, 6, parent);
    put16(&mut record, 10, kind);
    put32(&mut record, 12, flags);
    record.extend_from_slice(name);
    let end = record.len() as u16;
    // EOD and the last eight ordinary/LVAL start offsets equal the actual end.
    // Name has slot 0/start 32; every other variable carrier is genuinely NULL.
    for _ in 0..9 {
        record.extend_from_slice(&end.to_le_bytes());
    }
    record.extend_from_slice(&32_u16.to_le_bytes());
    record.extend_from_slice(&9_u16.to_le_bytes());
    record.extend_from_slice(&[0x1f, 0x60]);
    record
}

/// Independently authored complete images for a native17 catalog table rename.
/// Both preserve native object Id 7, catalog slot 8, TD7 and original user DATA8.
pub struct RenamePair {
    pub old_name: String,
    pub new_name: String,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub rows: Vec<Vec<i32>>,
}

/// Build both complete images from literal catalog facts, never a mutation codec.
/// Header 5/6 selects the retained ACE16 storage headers, not an Access UI release.
pub fn rename_pair(header: u8, old_name: &str, new_name: &str, populated: bool) -> RenamePair {
    assert!(matches!(header, 5 | 6));
    for name in [old_name, new_name] {
        assert!(!name.is_empty());
        assert!(name.encode_utf16().count() <= 64);
    }
    let original = canonical_creation::source(if populated {
        canonical_creation::Case::One
    } else {
        canonical_creation::Case::Empty
    });
    let before = named_image(header, old_name, &original.bytes);
    let mut after = named_image(header, new_name, &original.bytes);
    let slot = 4 * 4096 + 14 + 2 * 8;
    let old_start = 4 * 4096
        + usize::from(u16::from_le_bytes(
            before[slot..slot + 2].try_into().unwrap(),
        ));
    let new_start = 4 * 4096
        + usize::from(u16::from_le_bytes(
            after[slot..slot + 2].try_into().unwrap(),
        ));
    if new_start > old_start {
        // Vacated source bytes are deliberately retained, not zeroed or reused.
        after[old_start..new_start].copy_from_slice(&before[old_start..new_start]);
    }
    RenamePair {
        old_name: old_name.into(),
        new_name: new_name.into(),
        before,
        after,
        rows: original.rows,
    }
}

/// Author the first invalid stored-name width without weakening the valid rename factory.
/// This malformed source is for refusal checks, never a qualified original or AFTER.
pub fn overlong_name_source(header: u8, name: &str) -> Vec<u8> {
    assert!(matches!(header, 5 | 6));
    assert_eq!(name.encode_utf16().count(), 65);
    let original = canonical_creation::source(canonical_creation::Case::Empty);
    named_image(header, name, &original.bytes)
}

fn named_image(header: u8, name: &str, original: &[u8]) -> Vec<u8> {
    // NO_CLONE: Each independent raw oracle image is mutated separately from the retained
    // original; borrowing or shared immutable storage cannot supply these authored bytes.
    let mut bytes = original.to_vec();
    let mut payload = Vec::new();
    if name.starts_with('\u{feff}') {
        // Literal catalog escape: BOM marker, uncompressed mode, original units.
        payload.extend_from_slice(&[0xff, 0xfe, 0]);
    }
    payload.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog_page(&payload));
    let mut bytes = canonical_native17_creation::source(bytes);
    let page = &bytes[4 * 4096..5 * 4096];
    let mut records = Vec::new();
    let mut end = 4096;
    for slot in 0..8 {
        let at = 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
        records.push(page[start..end].to_vec());
        end = start;
    }
    records.insert(
        7,
        database_properties::native17_database_record(
            &database_properties::name_auto_correct_disabled(),
        ),
    );
    bytes[4 * 4096..5 * 4096].copy_from_slice(&canonical_native17_creation::packed(2, records));
    put32(&mut bytes, 2 * 4096 + 16, 9);
    bytes[20] = header;
    bytes
}

/// Author an inert catalog with one retired variable slot carrying opaque bytes.
/// The seventeen visible descriptors remain unchanged. The stored row geometry
/// instead retains eighteen column numbers and twelve variable slots; only the
/// user record has number 17 present. Generic catalog readers can still inspect
/// the visible core fields, but visible columns cannot prove this payload absent.
pub fn retired_variable_source(header: u8) -> Vec<u8> {
    let mut bytes = rename_pair(header, "Items", "Renamed", false).before;
    let page = &bytes[4 * 4096..5 * 4096];
    let mut records = Vec::new();
    let mut end = 4096;
    for slot in 0..9 {
        let at = 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
        let original = &page[start..end];
        assert_eq!(u16::from_le_bytes(original[..2].try_into().unwrap()), 17);
        let count_position = original.len() - 3 - 2;
        assert_eq!(
            u16::from_le_bytes(
                original[count_position..count_position + 2]
                    .try_into()
                    .unwrap()
            ),
            11
        );
        let footer = count_position - 12 * 2;
        let mut offsets = Vec::new();
        for variable in 0..=11 {
            let at = count_position - 2 * (variable + 1);
            offsets.push(u16::from_le_bytes(original[at..at + 2].try_into().unwrap()));
        }
        assert_eq!(usize::from(offsets[11]), footer);
        let mut record = original[..footer].to_vec();
        if slot == 8 {
            // Independent inline-LVAL-shaped payload in a slot with no surviving
            // column descriptor; no production payload or record codec is used.
            record.extend_from_slice(&0x8000_0004_u32.to_le_bytes());
            record.extend_from_slice(&[0; 8]);
            record.extend_from_slice(b"TCEX");
        }
        offsets.push(u16::try_from(record.len()).unwrap());
        for offset in offsets.into_iter().rev() {
            record.extend_from_slice(&offset.to_le_bytes());
        }
        record.extend_from_slice(&12_u16.to_le_bytes());
        let mut mask: [u8; 3] = original[original.len() - 3..].try_into().unwrap();
        if slot == 8 {
            mask[2] |= 0x02;
        }
        record.extend_from_slice(&mask);
        put16(&mut record, 0, 18);
        records.push(record);
        end = start;
    }
    bytes[4 * 4096..5 * 4096].copy_from_slice(&canonical_native17_creation::packed(2, records));
    put16(&mut bytes, 2 * 4096 + 41, 18);
    put16(&mut bytes, 2 * 4096 + 43, 12);
    bytes
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    fn row(bytes: &[u8], slot: usize) -> &[u8] {
        let page = &bytes[4 * 4096..5 * 4096];
        let at = 14 + 2 * slot;
        let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
        let end = if slot == 0 {
            4096
        } else {
            usize::from(u16::from_le_bytes(page[at - 2..at].try_into().unwrap()))
        };
        &page[start..end]
    }

    #[test]
    fn rename_pairs_keep_native_id_slots_definitions_rows_and_explicit_database_settings() {
        for header in [5, 6] {
            for populated in [false, true] {
                for (old, new) in [
                    ("Items", "T"),
                    ("T", "Customer contact records"),
                    ("Items", "連絡先 Ω🙂"),
                    ("連絡先 Ω🙂", "B"),
                    ("MixedCase", "mixedcase"),
                    ("Items", "\u{feff}āΩ雪🙂A"),
                ] {
                    let pair = rename_pair(header, old, new, populated);
                    assert_eq!(pair.before.len(), 9 * 4096);
                    assert_eq!(pair.after.len(), pair.before.len());
                    assert!(
                        pair.before[..4 * 4096] == pair.after[..4 * 4096],
                        "unrelated prefix changed"
                    );
                    assert!(
                        pair.before[5 * 4096..] == pair.after[5 * 4096..],
                        "definitions or user data changed"
                    );
                    for slot in 0..8 {
                        assert!(
                            row(&pair.before, slot) == row(&pair.after, slot),
                            "unrelated catalog row changed"
                        );
                    }
                    for bytes in [&pair.before, &pair.after] {
                        let user = row(bytes, 8);
                        assert_eq!(&user[2..6], &7_u32.to_le_bytes());
                        let database = row(bytes, 7);
                        assert_eq!(&database[2..6], &0x0f00_0004_u32.to_le_bytes());
                        assert_eq!(&database[6..10], &0x0f00_0002_u32.to_le_bytes());
                        assert_eq!(&database[10..12], &2_u16.to_le_bytes());
                        assert_eq!(&database[28..32], &0x8000_0000_u32.to_le_bytes());
                    }
                    assert_eq!(pair.rows.len(), usize::from(populated));
                }
            }
        }
    }

    #[test]
    fn full_unit_boundary_and_shrink_preserve_vacated_source_bytes() {
        let long = "表".repeat(64);
        let pair = rename_pair(5, &long, "I", true);
        let at = 4 * 4096 + 30;
        let old = 4 * 4096
            + usize::from(u16::from_le_bytes(
                pair.before[at..at + 2].try_into().unwrap(),
            ));
        let new = 4 * 4096
            + usize::from(u16::from_le_bytes(
                pair.after[at..at + 2].try_into().unwrap(),
            ));
        assert!(new > old);
        assert!(
            pair.before[old..new] == pair.after[old..new],
            "vacated bytes were cleared"
        );
        let growing = rename_pair(6, "I", &long, false);
        assert_eq!(growing.new_name.encode_utf16().count(), 64);
    }

    #[test]
    fn retired_variable_geometry_keeps_visible_descriptors_and_hidden_payload_distinct() {
        for header in [5, 6] {
            let source = retired_variable_source(header);
            let definition = &source[2 * 4096..3 * 4096];
            assert_eq!(&definition[41..47], &[18, 0, 12, 0, 17, 0]);
            for slot in 0..9 {
                let record = row(&source, slot);
                assert_eq!(&record[..2], &[18, 0]);
                let count = record.len() - 5;
                assert_eq!(&record[count..count + 2], &[12, 0]);
                let start = usize::from(u16::from_le_bytes(
                    record[count - 24..count - 22].try_into().unwrap(),
                ));
                let end = usize::from(u16::from_le_bytes(
                    record[count - 26..count - 24].try_into().unwrap(),
                ));
                assert_eq!(record[record.len() - 1] & 0x02 != 0, slot == 8);
                if slot == 8 {
                    assert_eq!(&record[start..start + 4], &0x8000_0004_u32.to_le_bytes());
                    assert_eq!(&record[start + 4..start + 12], &[0; 8]);
                    assert_eq!(&record[start + 12..end], b"TCEX");
                } else {
                    assert_eq!(start, end);
                }
            }
        }
    }
}
