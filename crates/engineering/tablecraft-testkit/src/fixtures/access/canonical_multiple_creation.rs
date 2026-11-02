//! Independent multiple-table SOURCEs built from qualified repository originals.
use super::{canonical_creation, canonical_variable_creation, data_page, put16, put32};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// One closed complete-image recipe, not a production request or plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Empty two-Int32 and five-variable-profile tables with different schemas.
    TwoEmpty,
    /// Two populated tables share Id names and row ordinals without sharing data.
    Populated,
    /// A full fixed DATA page and an empty variable page have different hints.
    CandidateBits,
    /// Sixteen independently named empty tables exercise every appended role.
    Sixteen,
}

/// Frozen source order, independent of a production constructor's output.
pub const CASES: [Case; 4] = [
    Case::TwoEmpty,
    Case::Populated,
    Case::CandidateBits,
    Case::Sixteen,
];

impl Case {
    /// Stable export and observation identity.
    pub const fn id(self) -> &'static str {
        match self {
            Self::TwoEmpty => "two-empty",
            Self::Populated => "populated",
            Self::CandidateBits => "candidate-bits",
            Self::Sixteen => "sixteen",
        }
    }
}

/// Literal native declaration facts, not a fabricated production NativeColumn.
pub struct Column {
    /// Exact original field name.
    pub name: String,
    /// Native type byte.
    pub native_type: u8,
    /// Declared storage width; Boolean has no payload bytes.
    pub width: u16,
    /// Fixed offset or an ordinary variable field.
    pub fixed_offset: Option<u16>,
    /// Native variable-table index, including fixed fields' unused index.
    pub variable_index: u16,
    /// NUMERIC precision and retained scale, if present.
    pub decimal: Option<(u8, u8)>,
}

/// Exact selected table and physical role ledger.
pub struct Table {
    /// Catalog name, distinct from the raw declaration names.
    pub name: String,
    /// Independently qualified native field facts.
    pub columns: Vec<Column>,
    /// Exact row carriers in this table's original source order.
    pub rows: Vec<Vec<Value>>,
    /// Actual catalog native identity and TD page.
    pub definition_page: u32,
    /// Independently owned DATA page.
    pub data_page: u32,
    /// Actual owned map row on map-container page 3.
    pub owned_slot: u8,
    /// Actual candidate map row on map-container page 3.
    pub candidate_slot: u8,
    /// Actual free DATA bytes after records and directory.
    pub free_bytes: u16,
    /// Whether this table's DATA page is a candidate.
    pub candidate: bool,
    /// Independently qualified original definition before explicit relocation.
    pub original_definition: Vec<u8>,
    /// Independently qualified original DATA before owner relocation.
    pub original_data: Vec<u8>,
}

/// Complete image and its independently named user tables.
pub struct Source {
    /// Closed recipe identity.
    pub case: Case,
    /// Complete owned image with seven common pages and two pages per user.
    pub bytes: Vec<u8>,
    /// Catalog order, which also determines page and appended-map order.
    pub tables: Vec<Table>,
    /// Exact ordered members of each map row on page 3.
    pub map_members: Vec<Option<u32>>,
}

struct Template {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    bytes: Vec<u8>,
    candidate: bool,
}

fn fixed(name: String, case: canonical_creation::Case) -> Template {
    let original = canonical_creation::source(case);
    let columns = original
        .columns
        .iter()
        .enumerate()
        .map(|(position, name)| Column {
            name: name.clone(),
            native_type: 4,
            width: 4,
            fixed_offset: Some((4 * position) as u16),
            variable_index: 0,
            decimal: None,
        })
        .collect();
    Template {
        name,
        columns,
        rows: original
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(Value::Int32).collect())
            .collect(),
        bytes: original.bytes,
        candidate: case != canonical_creation::Case::Wide,
    }
}

fn variable(name: &str, case: canonical_variable_creation::Case) -> Template {
    let original = canonical_variable_creation::source(case);
    Template {
        name: name.into(),
        columns: original
            .columns
            .iter()
            .map(|column| Column {
                name: column.name.into(),
                native_type: column.native_type,
                width: column.width,
                fixed_offset: column.fixed_offset,
                variable_index: column.variable_index,
                decimal: (column.native_type == 16).then_some((column.precision, column.scale)),
            })
            .collect(),
        rows: original.rows,
        bytes: original.bytes,
        candidate: true,
    }
}

/// Build complete independent bytes without invoking a creator, codec or reader.
pub fn source(case: Case) -> Source {
    let templates = match case {
        Case::TwoEmpty => vec![
            fixed("Numbers".into(), canonical_creation::Case::Empty),
            variable("Mixed Values", canonical_variable_creation::Case::Empty),
        ],
        Case::Populated => vec![
            fixed("Numbers".into(), canonical_creation::Case::Extrema),
            variable("Mixed Values", canonical_variable_creation::Case::Unicode),
        ],
        Case::CandidateBits => vec![
            fixed("Full Rows".into(), canonical_creation::Case::Wide),
            variable("Mixed Values", canonical_variable_creation::Case::Empty),
        ],
        Case::Sixteen => (1..=16)
            .map(|number| fixed(format!("T{number:02}"), canonical_creation::Case::Empty))
            .collect(),
    };
    let base = canonical_creation::source(canonical_creation::Case::Empty);
    let mut bytes = base.bytes[..7 * PAGE].to_vec();
    bytes.resize((7 + 2 * templates.len()) * PAGE, 0);
    let mut tables = Vec::new();
    for (index, template) in templates.into_iter().enumerate() {
        let definition_page = (7 + 2 * index) as u32;
        let data_page = definition_page + 1;
        let (owned_slot, candidate_slot) = if index == 0 {
            (3, 7)
        } else {
            ((16 + 2 * (index - 1)) as u8, (17 + 2 * (index - 1)) as u8)
        };
        let original_definition = template.bytes[7 * PAGE..8 * PAGE].to_vec();
        let original_data = template.bytes[8 * PAGE..9 * PAGE].to_vec();
        let free_bytes = u16::from_le_bytes(original_data[2..4].try_into().unwrap());
        let mut definition = original_definition.clone();
        assert_eq!(definition[40], 0x4e);
        put32(&mut definition, 55, (3 << 8) | u32::from(owned_slot));
        put32(&mut definition, 59, (3 << 8) | u32::from(candidate_slot));
        let mut data = original_data.clone();
        assert_eq!(&data[4..8], &7_u32.to_le_bytes());
        put32(&mut data, 4, definition_page);
        bytes[definition_page as usize * PAGE..(definition_page as usize + 1) * PAGE]
            .copy_from_slice(&definition);
        bytes[data_page as usize * PAGE..(data_page as usize + 1) * PAGE].copy_from_slice(&data);
        tables.push(Table {
            name: template.name,
            columns: template.columns,
            rows: template.rows,
            definition_page,
            data_page,
            owned_slot,
            candidate_slot,
            free_bytes,
            candidate: template.candidate,
            original_definition,
            original_data,
        });
    }

    // Original role rows 0..15 stay stable, including all four catalog LVAL pairs.
    let mut map_members = vec![None; 16 + 2 * (tables.len() - 1)];
    map_members[0] = Some(4);
    map_members[4] = Some(4);
    for table in &tables {
        map_members[usize::from(table.owned_slot)] = Some(table.data_page);
        map_members[usize::from(table.candidate_slot)] = table.candidate.then_some(table.data_page);
    }
    let records = map_members
        .iter()
        .map(|member| {
            let mut map = vec![0; 13];
            if let Some(number) = member {
                assert!(*number < 64);
                map[5 + *number as usize / 8] |= 1 << (*number % 8);
            }
            (0, map)
        })
        .collect::<Vec<_>>();
    let mut maps = data_page(0, &records);
    put16(&mut maps, 2, (4096 - 14 - 15 * records.len()) as u16);
    bytes[3 * PAGE..4 * PAGE].copy_from_slice(&maps);

    let systems = [
        (0x0f00_0000, 0, "DatabaseRoot", 3, 0x8000_0002),
        (0x0f00_0001, 0x0f00_0000, "Tables", 3, 0x8000_0002),
        (0x0f00_0002, 0x0f00_0000, "Databases", 3, 0x8000_0002),
        (0x0f00_0003, 0x0f00_0000, "Relationships", 3, 0x8000_0002),
        (2, 0x0f00_0001, "MSysObjects", 1, 0x8000_0002),
        (5, 0x0f00_0001, "MSysRelationships", 1, 0x8000_0002),
        (6, 0x0f00_0001, "MSysACEs", 1, 0x8000_0002),
    ];
    let mut catalog = systems
        .into_iter()
        .map(|(id, parent, name, kind, flags)| (0, catalog_record(id, parent, kind, flags, name)))
        .collect::<Vec<_>>();
    for table in &tables {
        catalog.push((
            0,
            catalog_record(table.definition_page, 0x0f00_0001, 1, 0, &table.name),
        ));
    }
    let used = catalog
        .iter()
        .map(|(_, record)| record.len() + 2)
        .sum::<usize>();
    let mut data = data_page(2, &catalog);
    put16(&mut data, 2, (4096 - 14 - used) as u16);
    bytes[4 * PAGE..5 * PAGE].copy_from_slice(&data);
    put32(&mut bytes, 2 * PAGE + 16, catalog.len() as u32);
    Source {
        case,
        bytes,
        tables,
        map_members,
    }
}

fn catalog_record(id: u32, parent: u32, kind: u16, flags: u32, name: &str) -> Vec<u8> {
    let mut record = vec![0; 32];
    put16(&mut record, 0, 15);
    put32(&mut record, 2, id);
    put32(&mut record, 6, parent);
    put16(&mut record, 10, kind);
    put32(&mut record, 12, flags);
    record.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    let end = record.len() as u16;
    for _ in 0..9 {
        record.extend_from_slice(&end.to_le_bytes());
    }
    record.extend_from_slice(&32_u16.to_le_bytes());
    record.extend_from_slice(&9_u16.to_le_bytes());
    record.extend_from_slice(&[0x1f, 0x60]);
    record
}
