//! Independent SOURCEs separating stored catalog Text from raw definition names.
use super::{canonical_creation, data_page, put16, put32};

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
    records.push((0, catalog_record(7, 0x0f00_0001, 1, 0, &payload)));
    let payload_bytes = records.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut catalog = data_page(2, &records);
    put16(&mut catalog, 2, (4096 - 14 - 2 * 8 - payload_bytes) as u16);
    bytes[4 * 4096..5 * 4096].copy_from_slice(&catalog);

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
