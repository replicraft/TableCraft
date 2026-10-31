//! Independently authored canonical ACE SOURCE goldens, not a production creator.
//!
//! Every page is constructed from the declared test facts below. The root prefix
//! uses our literal RC4 oracle over explicit clear fields, not an SDK template.
//! The first qualification compares all bytes with the frozen Python v2 sources.
use super::{data_page, put16, put32};

const PAGE: usize = 4096;

/// One of the six independently read-qualified source recipes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Empty ordinary two-column table.
    Empty,
    /// One ordinary row with values 1 and 10.
    One,
    /// Signed Int32 endpoint values.
    Extrema,
    /// The selected ceiling of 64 two-column rows.
    Boundary,
    /// A table and columns with spaces and a Unicode name.
    Named,
    /// Sixteen columns and 58 rows, leaving 22 DATA bytes free.
    Wide,
}

/// All independently qualified source recipes in their original order.
pub const CASES: [Case; 6] = [
    Case::Empty,
    Case::One,
    Case::Extrema,
    Case::Boundary,
    Case::Named,
    Case::Wide,
];

impl Case {
    /// Stable name of the corresponding frozen Python source artifact.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::One => "one",
            Self::Extrema => "extrema",
            Self::Boundary => "boundary",
            Self::Named => "named",
            Self::Wide => "wide",
        }
    }
}

/// Complete independent source bytes and the typed facts expected by consumers.
pub struct Source {
    /// Exact ordinary user-table name.
    pub table: &'static str,
    /// Ordered ordinary Int32 field names.
    pub columns: Vec<String>,
    /// Original rows in physical slot order, with no nullable cells.
    pub rows: Vec<Vec<i32>>,
    /// Complete nine-page image, including system tables and allocation maps.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Field<'a> {
    name: &'a str,
    kind: u8,
    width: u16,
    fixed: Option<u16>,
    variable: u16,
}

impl<'a> Field<'a> {
    const fn fixed(name: &'a str, kind: u8, width: u16, offset: u16, variable: u16) -> Self {
        Self {
            name,
            kind,
            width,
            fixed: Some(offset),
            variable,
        }
    }

    const fn variable(name: &'a str, kind: u8, width: u16, slot: u16) -> Self {
        Self {
            name,
            kind,
            width,
            fixed: None,
            variable: slot,
        }
    }
}

const CATALOG: [Field<'static>; 15] = [
    Field::fixed("Id", 4, 4, 0, 0),
    Field::fixed("ParentId", 4, 4, 4, 0),
    Field::variable("Name", 10, 510, 0),
    Field::fixed("Type", 3, 2, 8, 1),
    Field::fixed("Flags", 4, 4, 10, 1),
    Field::variable("LvProp", 11, 0, 5),
    Field::variable("Lv", 11, 0, 6),
    Field::variable("LvExtra", 11, 0, 7),
    Field::variable("LvModule", 11, 0, 8),
    Field::variable("Database", 10, 510, 1),
    Field::variable("ForeignName", 10, 510, 2),
    Field::variable("Connect", 10, 510, 3),
    Field::variable("Owner", 9, 255, 4),
    Field::fixed("DateCreate", 8, 8, 14, 5),
    Field::fixed("DateUpdate", 8, 8, 22, 5),
];

const RELATIONSHIPS: [Field<'static>; 8] = [
    Field::variable("szRelationship", 10, 510, 0),
    Field::fixed("grbit", 4, 4, 0, 1),
    Field::fixed("ccolumn", 4, 4, 4, 1),
    Field::fixed("icolumn", 4, 4, 8, 1),
    Field::variable("szReferencedObject", 10, 510, 1),
    Field::variable("szObject", 10, 510, 2),
    Field::variable("szReferencedColumn", 10, 510, 3),
    Field::variable("szColumn", 10, 510, 4),
];

const ACES: [Field<'static>; 4] = [
    Field::fixed("ACM", 4, 4, 0, 0),
    Field::fixed("FInheritable", 1, 0, 0, 0),
    Field::fixed("ObjectId", 4, 4, 4, 0),
    Field::variable("SID", 9, 255, 0),
];

fn utf16(name: &str) -> Vec<u8> {
    name.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn header() -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..4].copy_from_slice(&[0, 1, 0, 0]);
    page[4..20].copy_from_slice(b"Standard ACE DB\0");
    page[20] = 5;
    let mut clear = [0; 128];
    put16(&mut clear, 60 - 24, 1252);
    // Clear page codec, creation DATE and password are all explicitly zero.
    clear[110 - 24..114 - 24].copy_from_slice(&[9, 4, 0, 1]);

    let key = [0xc7_u8, 0xda, 0x39, 0x6b];
    let mut state: [u8; 256] = std::array::from_fn(|index| index as u8);
    let mut second = 0;
    for first in 0..256 {
        second = (second + usize::from(state[first]) + usize::from(key[first % 4])) % 256;
        state.swap(first, second);
    }
    let mut first = 0;
    second = 0;
    for byte in &mut clear {
        first = (first + 1) % 256;
        second = (second + usize::from(state[first])) % 256;
        state.swap(first, second);
        let selected = (usize::from(state[first]) + usize::from(state[second])) % 256;
        *byte ^= state[selected];
    }
    page[24..152].copy_from_slice(&clear);
    assert_eq!(&page[62..66], &[0xfb, 0x8a, 0xbc, 0x4e]);
    page
}

fn definition(fields: &[Field<'_>], rows: usize, map_slot: u8) -> Vec<u8> {
    let mut page = vec![0; PAGE];
    page[..2].copy_from_slice(&[2, 1]);
    put32(&mut page, 12, 1625);
    put32(&mut page, 16, rows as u32);
    page[24] = 1;
    page[40] = if map_slot == 3 { 0x4e } else { 0x53 };
    put16(&mut page, 41, fields.len() as u16);
    put16(
        &mut page,
        43,
        fields.iter().filter(|field| field.fixed.is_none()).count() as u16,
    );
    put16(&mut page, 45, fields.len() as u16);
    put32(&mut page, 55, (3 << 8) | u32::from(map_slot));
    put32(&mut page, 59, (3 << 8) | u32::from(map_slot + 4));
    let mut names = 63 + 25 * fields.len();
    for (number, field) in fields.iter().enumerate() {
        let at = 63 + 25 * number;
        page[at] = field.kind;
        put32(&mut page, at + 1, 1625);
        put16(&mut page, at + 5, number as u16);
        put16(&mut page, at + 7, field.variable);
        put16(&mut page, at + 9, number as u16);
        if field.kind == 10 {
            page[at + 11..at + 15].copy_from_slice(&[9, 4, 0, 1]);
        }
        page[at + 15] = if field.fixed.is_some() { 3 } else { 2 };
        if let Some(offset) = field.fixed {
            put16(&mut page, at + 21, offset);
        }
        put16(&mut page, at + 23, field.width);
        let encoded = utf16(field.name);
        put16(&mut page, names, encoded.len() as u16);
        names += 2;
        page[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    if map_slot == 0 {
        for (index, column) in [5_u16, 6, 7, 8].into_iter().enumerate() {
            put16(&mut page, names, column);
            put32(&mut page, names + 2, (3 << 8) | (8 + 2 * index as u32));
            put32(&mut page, names + 6, (3 << 8) | (9 + 2 * index as u32));
            names += 10;
        }
    }
    put16(&mut page, names, 0xffff);
    let length = names + 2;
    put32(&mut page, 8, length as u32);
    put16(&mut page, 2, (PAGE - length - 8) as u16);
    page
}

fn packed_page(owner: u32, records: Vec<Vec<u8>>) -> Vec<u8> {
    let count = records.len();
    let payload = records.iter().map(Vec::len).sum::<usize>();
    let rows = records.into_iter().map(|row| (0, row)).collect::<Vec<_>>();
    let mut page = data_page(owner, &rows);
    put16(&mut page, 2, (PAGE - 14 - 2 * count - payload) as u16);
    page
}

fn inline_map(member: Option<u8>) -> Vec<u8> {
    let mut record = vec![0; 13];
    if let Some(page) = member {
        record[5 + usize::from(page) / 8] |= 1 << (page % 8);
    }
    record
}

fn catalog_record(id: u32, parent: u32, name: &str, kind: u16, flags: u32) -> Vec<u8> {
    let mut row = vec![0; 32];
    put16(&mut row, 0, 15);
    put32(&mut row, 2, id);
    put32(&mut row, 6, parent);
    put16(&mut row, 10, kind);
    put32(&mut row, 12, flags);
    // Both present zero DATE payloads occupy 16..32; only Name is a present
    // variable. Every link/owner/opaque/LVAL payload is explicitly NULL.
    row.extend(utf16(name));
    let end = row.len() as u16;
    for _ in 0..9 {
        row.extend(end.to_le_bytes());
    }
    row.extend(32_u16.to_le_bytes());
    row.extend(9_u16.to_le_bytes());
    row.extend([0x1f, 0x60]);
    row
}

fn user_record(values: &[i32]) -> Vec<u8> {
    let mut row = (values.len() as u16).to_le_bytes().to_vec();
    for value in values {
        row.extend(value.to_le_bytes());
    }
    let present = (1_u32 << values.len()) - 1;
    let mask = present.to_le_bytes();
    row.extend_from_slice(&mask[..values.len().div_ceil(8)]);
    row
}

/// Build only a literal independent SOURCE recipe; no production writer is used.
pub fn source(case: Case) -> Source {
    let (table, columns, rows): (&str, Vec<String>, Vec<Vec<i32>>) = match case {
        Case::Empty => ("Items", vec!["Id".into(), "Score".into()], Vec::new()),
        Case::One => (
            "Items",
            vec!["Id".into(), "Score".into()],
            vec![vec![1, 10]],
        ),
        Case::Extrema => (
            "Items",
            vec!["Id".into(), "Score".into()],
            vec![vec![1, i32::MIN], vec![2, i32::MAX]],
        ),
        Case::Boundary => (
            "Items",
            vec!["Id".into(), "Score".into()],
            (1..=64).map(|number| vec![number, number * 10]).collect(),
        ),
        Case::Named => (
            "People Stats",
            vec!["Person Id".into(), "Score Δ".into()],
            vec![vec![7, 42]],
        ),
        Case::Wide => (
            "Wide Rows",
            (1..=16).map(|column| format!("C{column:02}")).collect(),
            (1..=58)
                .map(|number| (1..=16).map(|column| number * 100 + column).collect())
                .collect(),
        ),
    };
    let user_fields = columns
        .iter()
        .enumerate()
        .map(|(number, name)| Field::fixed(name, 4, 4, (4 * number) as u16, 0))
        .collect::<Vec<_>>();
    let mut pages: [Vec<u8>; 9] = std::array::from_fn(|_| Vec::new());
    pages[0] = header();
    pages[1] = packed_page(1, vec![inline_map(None)]);
    pages[2] = definition(&CATALOG, 8, 0);
    let candidate = if case == Case::Wide { None } else { Some(8) };
    let mut maps = [Some(4), None, None, Some(8), Some(4), None, None, candidate]
        .into_iter()
        .map(inline_map)
        .collect::<Vec<_>>();
    maps.extend((0..8).map(|_| inline_map(None)));
    pages[3] = packed_page(0, maps);
    let objects = [
        (0x0f000000, 0, "DatabaseRoot", 3, 0x80000002),
        (0x0f000001, 0x0f000000, "Tables", 3, 0x80000002),
        (0x0f000002, 0x0f000000, "Databases", 3, 0x80000002),
        (0x0f000003, 0x0f000000, "Relationships", 3, 0x80000002),
        (2, 0x0f000001, "MSysObjects", 1, 0x80000002),
        (5, 0x0f000001, "MSysRelationships", 1, 0x80000002),
        (6, 0x0f000001, "MSysACEs", 1, 0x80000002),
        (7, 0x0f000001, table, 1, 0),
    ];
    pages[4] = packed_page(
        2,
        objects
            .into_iter()
            .map(|(id, parent, name, kind, flags)| catalog_record(id, parent, name, kind, flags))
            .collect(),
    );
    pages[5] = definition(&RELATIONSHIPS, 0, 1);
    pages[6] = definition(&ACES, 0, 2);
    pages[7] = definition(&user_fields, rows.len(), 3);
    pages[8] = packed_page(7, rows.iter().map(|row| user_record(row)).collect());
    let bytes = pages.concat();
    assert_eq!(bytes.len(), 9 * PAGE);
    Source {
        table,
        columns,
        rows,
        bytes,
    }
}
