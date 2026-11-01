//! Independent complete-image cases for a proposed ordinary-content grammar.
//!
//! These are read sources and explicit grammar hypotheses, not absence tokens,
//! creator provenance, normal plans or native Access compatibility evidence.
use super::{
    canonical_before_change, canonical_creation, canonical_fixed_creation,
    canonical_multiple_creation, canonical_variable_creation, put16, put32, wide_properties,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const TABLES_ID: u32 = 0x0f00_0001;
const CATALOG_PAGE: usize = 4;
const MAP_PAGE: usize = 3;
const CATALOG_FIELDS: u16 = 15;
const CATALOG_VARIABLES: u16 = 9;

/// Finite independent source identities, with no production admission call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Ordinary present Int32 values at both signed endpoints.
    Extrema,
    /// Spaces and Unicode in user metadata are parameters, not provenance.
    Named,
    /// AutoExec is an ordinary local user-table name, not an object-kind change.
    ProgramLikeName,
    /// Short Text and Binary contain XML-looking ordinary user values.
    XmlData,
    /// A NULL fixed Int64 retains eight XML-looking inactive bytes.
    InactiveXml,
    /// Two populated independent schemas, including short variable values.
    Multiple,
    /// Sixteen ordinary tables retain appended map roles and catalog entries.
    Sixteen,
    /// Every live catalog record is reordered without changing its identity.
    ReorderedCatalog,
    /// TD10/DATA9 replace TD7/DATA8; the vacated in-file pages are free and zero.
    RelocatedUser,
    /// Map container12 replaces3; all declaration and LVAL references follow it.
    RelocatedMaps,
    /// Two DATA pages belong to one table, in source page/slot order.
    SplitData,
    /// One extra zero page is explicitly GLOBAL-free, not an orphan allocation.
    TrailingFreePage,
    /// Actual positive native event content cannot be relabeled as absent.
    PositiveProgram,
    /// A complete opaque MR2 payload is present in catalog LvExtra.
    OpaqueExtra,
    /// Present-empty inline LvExtra is distinct from a NULL variable span.
    PresentEmptyExtra,
    /// A physically retained deleted catalog slot is not live-inventory absence.
    DeletedCatalogSlot,
    /// A zero-filled allocated page has no admitted representation role.
    OrphanZeroPage,
    /// An unclaimed allocated page contains XML-looking bytes.
    OrphanPayloadPage,
    /// A GLOBAL-free page contains unexplained nonzero bytes.
    NonzeroFreePage,
    /// Owned and candidate map references alias the same physical map cell.
    MapAlias,
    /// Distinct system owners share one empty owned-map cell.
    SharedSystemMap,
    /// A different full32 object identity aliases the same low24 TD page.
    Low24DefinitionAlias,
    /// A catalog row does not actually store all fifteen fields.
    ShortCatalogRow,
    /// All fields are claimed but the variable footer stores only eight slots.
    ShortCatalogFooter,
    /// Catalog OLE declarations have no required LVAL map-pair tail.
    MissingLvalPairs,
    /// Two catalog OLE declarations share one owned-map cell.
    DuplicateLvalMap,
    /// An unexplained header byte is not declared user data.
    UnknownHeaderByte,
    /// An unexplained DATA free-region byte is not declared user data.
    UnknownFreeByte,
    /// A trailing byte is outside complete native page geometry.
    ExtraEofByte,
}

/// Frozen authoring order. Eligibility is a hypothesis until a real gate passes.
pub const CASES: [Case; 29] = [
    Case::Extrema,
    Case::Named,
    Case::ProgramLikeName,
    Case::XmlData,
    Case::InactiveXml,
    Case::Multiple,
    Case::Sixteen,
    Case::ReorderedCatalog,
    Case::RelocatedUser,
    Case::RelocatedMaps,
    Case::SplitData,
    Case::TrailingFreePage,
    Case::PositiveProgram,
    Case::OpaqueExtra,
    Case::PresentEmptyExtra,
    Case::DeletedCatalogSlot,
    Case::OrphanZeroPage,
    Case::OrphanPayloadPage,
    Case::NonzeroFreePage,
    Case::MapAlias,
    Case::SharedSystemMap,
    Case::Low24DefinitionAlias,
    Case::ShortCatalogRow,
    Case::ShortCatalogFooter,
    Case::MissingLvalPairs,
    Case::DuplicateLvalMap,
    Case::UnknownHeaderByte,
    Case::UnknownFreeByte,
    Case::ExtraEofByte,
];

impl Case {
    /// Stable source identity for explicit later export/observations.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Extrema => "extrema",
            Self::Named => "named",
            Self::ProgramLikeName => "program-like-name",
            Self::XmlData => "xml-data",
            Self::InactiveXml => "inactive-xml",
            Self::Multiple => "multiple",
            Self::Sixteen => "sixteen",
            Self::ReorderedCatalog => "reordered-catalog",
            Self::RelocatedUser => "relocated-user",
            Self::RelocatedMaps => "relocated-maps",
            Self::SplitData => "split-data",
            Self::TrailingFreePage => "trailing-free-page",
            Self::PositiveProgram => "positive-program",
            Self::OpaqueExtra => "opaque-extra",
            Self::PresentEmptyExtra => "present-empty-extra",
            Self::DeletedCatalogSlot => "deleted-catalog-slot",
            Self::OrphanZeroPage => "orphan-zero-page",
            Self::OrphanPayloadPage => "orphan-payload-page",
            Self::NonzeroFreePage => "nonzero-free-page",
            Self::MapAlias => "map-alias",
            Self::SharedSystemMap => "shared-system-map",
            Self::Low24DefinitionAlias => "low24-definition-alias",
            Self::ShortCatalogRow => "short-catalog-row",
            Self::ShortCatalogFooter => "short-catalog-footer",
            Self::MissingLvalPairs => "missing-lval-pairs",
            Self::DuplicateLvalMap => "duplicate-lval-map",
            Self::UnknownHeaderByte => "unknown-header-byte",
            Self::UnknownFreeByte => "unknown-free-byte",
            Self::ExtraEofByte => "extra-eof-byte",
        }
    }
}

/// Actual typed user observations, independent of a mutation planner.
pub struct Table {
    /// Catalog table name; role does not come from its spelling.
    pub name: String,
    /// Ordered field names inherited from the qualified original declaration.
    pub columns: Vec<String>,
    /// Logical values; inactive physical NULL bytes are not decoded as values.
    pub rows: Vec<Vec<Value>>,
    /// Actual physical definition page, including relocation cases.
    pub definition: u32,
    /// Actual DATA ownership in source scan order.
    pub data: Vec<u32>,
}

/// Authored input and expected grammar disposition, not a capability object.
pub struct Source {
    /// Complete owned input, including explicitly malformed negative tails.
    pub bytes: Vec<u8>,
    /// Expected live ordinary table observations, where native reads can succeed.
    pub tables: Vec<Table>,
    /// Inside the proposed complete grammar; never a serialized absence claim.
    pub ordinary: bool,
}

fn fixed(case: canonical_creation::Case) -> Source {
    let original = canonical_creation::source(case);
    Source {
        bytes: original.bytes,
        tables: vec![Table {
            name: original.table.into(),
            columns: original.columns,
            rows: original
                .rows
                .into_iter()
                .map(|row| row.into_iter().map(Value::Int32).collect())
                .collect(),
            definition: 7,
            data: vec![8],
        }],
        ordinary: true,
    }
}

fn multiple(case: canonical_multiple_creation::Case) -> Source {
    let original = canonical_multiple_creation::source(case);
    Source {
        bytes: original.bytes,
        tables: original
            .tables
            .into_iter()
            .map(|table| Table {
                name: table.name,
                columns: table
                    .columns
                    .into_iter()
                    .map(|column| column.name)
                    .collect(),
                rows: table.rows,
                definition: table.definition_page,
                data: vec![table.data_page],
            })
            .collect(),
        ordinary: true,
    }
}

fn row_bounds(bytes: &[u8], page: usize, slot: usize) -> (usize, usize) {
    let base = page * PAGE;
    let count = usize::from(u16::from_le_bytes(
        bytes[base + 12..base + 14].try_into().unwrap(),
    ));
    assert!(slot < count);
    let word = base + 14 + 2 * slot;
    let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(start & 0xe000, 0);
    let end = if slot == 0 {
        PAGE
    } else {
        usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    (base + usize::from(start), base + end)
}

fn records(bytes: &[u8], page: usize) -> Vec<Vec<u8>> {
    let at = page * PAGE + 12;
    let count = usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()));
    (0..count)
        .map(|slot| {
            let (start, end) = row_bounds(bytes, page, slot);
            bytes[start..end].to_vec()
        })
        .collect()
}

fn packed(owner: u32, records: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let used = records.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
    assert!(14 + used <= PAGE);
    let mut page = super::data_page(owner, records);
    put16(&mut page, 2, (PAGE - 14 - used) as u16);
    page
}

fn replace_catalog(source: &mut Source, rows: Vec<(u16, Vec<u8>)>, live: usize) {
    source.bytes[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE].copy_from_slice(&packed(2, &rows));
    put32(&mut source.bytes, 2 * PAGE + 16, live as u32);
}

fn catalog_row(id: u32, name: &str) -> Vec<u8> {
    let mut row = vec![0; 32];
    put16(&mut row, 0, CATALOG_FIELDS);
    put32(&mut row, 2, id);
    put32(&mut row, 6, TABLES_ID);
    put16(&mut row, 10, 1);
    row.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    let end = row.len() as u16;
    for _ in 0..9 {
        row.extend_from_slice(&end.to_le_bytes());
    }
    row.extend_from_slice(&32_u16.to_le_bytes());
    row.extend_from_slice(&CATALOG_VARIABLES.to_le_bytes());
    row.extend_from_slice(&[0x1f, 0x60]);
    row
}

fn extra_row(original: &[u8], payload: &[u8]) -> Vec<u8> {
    assert_eq!(&original[32..42], b"I\0t\0e\0m\0s\0");
    let mut inline = (0x8000_0000_u32 | payload.len() as u32)
        .to_le_bytes()
        .to_vec();
    inline.extend_from_slice(&[0; 8]);
    inline.extend_from_slice(payload);
    let mut row = original[..32].to_vec();
    let values: [Option<&[u8]>; 9] = [
        Some(&original[32..42]),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&inline),
        None,
    ];
    let mut starts = [0_u16; 9];
    for (start, value) in starts.iter_mut().zip(values) {
        *start = row.len() as u16;
        if let Some(value) = value {
            row.extend_from_slice(value);
        }
    }
    row.extend_from_slice(&(row.len() as u16).to_le_bytes());
    for start in starts.into_iter().rev() {
        row.extend_from_slice(&start.to_le_bytes());
    }
    row.extend_from_slice(&9_u16.to_le_bytes());
    row.extend_from_slice(&[0x9f, 0x60]);
    row
}

fn map_members(bytes: &mut [u8], container: usize, slot: usize, pages: &[u32]) {
    let (start, end) = row_bounds(bytes, container, slot);
    assert_eq!(end - start, 13);
    assert_eq!(&bytes[start..start + 5], &[0; 5]);
    bytes[start + 5..end].fill(0);
    for &page in pages {
        assert!(page < 64);
        bytes[start + 5 + page as usize / 8] |= 1 << (page % 8);
    }
}

fn free_page(bytes: &mut [u8], page: u32) {
    let (start, end) = row_bounds(bytes, 1, 0);
    assert_eq!(end - start, 13);
    assert!(page < 64);
    bytes[start + 5 + page as usize / 8] |= 1 << (page % 8);
}

fn relocated(maps_too: bool) -> Source {
    let mut source = fixed(canonical_creation::Case::One);
    let definition = source.bytes[7 * PAGE..8 * PAGE].to_vec();
    let mut data = source.bytes[8 * PAGE..9 * PAGE].to_vec();
    put32(&mut data, 4, 10);
    source.bytes.resize(11 * PAGE, 0);
    source.bytes[7 * PAGE..9 * PAGE].fill(0);
    source.bytes[9 * PAGE..10 * PAGE].copy_from_slice(&data);
    source.bytes[10 * PAGE..11 * PAGE].copy_from_slice(&definition);
    let (catalog, _) = row_bounds(&source.bytes, 4, 7);
    assert_eq!(
        &source.bytes[catalog + 2..catalog + 6],
        &7_u32.to_le_bytes()
    );
    put32(&mut source.bytes, catalog + 2, 10);
    map_members(&mut source.bytes, MAP_PAGE, 3, &[9]);
    map_members(&mut source.bytes, MAP_PAGE, 7, &[9]);
    free_page(&mut source.bytes, 7);
    free_page(&mut source.bytes, 8);
    source.tables[0].definition = 10;
    source.tables[0].data = vec![9];
    if maps_too {
        let maps = source.bytes[3 * PAGE..4 * PAGE].to_vec();
        source.bytes.resize(13 * PAGE, 0);
        source.bytes[12 * PAGE..13 * PAGE].copy_from_slice(&maps);
        source.bytes[3 * PAGE..4 * PAGE].fill(0);
        for definition in [2_usize, 5, 6, 10] {
            for at in [55, 59] {
                let offset = definition * PAGE + at;
                let old = u32::from_le_bytes(source.bytes[offset..offset + 4].try_into().unwrap());
                assert_eq!(old >> 8, 3);
                put32(&mut source.bytes, offset, (12 << 8) | (old & 0xff));
            }
        }
        let length = u32::from_le_bytes(
            source.bytes[2 * PAGE + 8..2 * PAGE + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        let tail = 2 * PAGE + length - 42;
        for pair in 0..4 {
            for field in [2, 6] {
                let offset = tail + pair * 10 + field;
                let old = u32::from_le_bytes(source.bytes[offset..offset + 4].try_into().unwrap());
                assert_eq!(old >> 8, 3);
                put32(&mut source.bytes, offset, (12 << 8) | (old & 0xff));
            }
        }
        free_page(&mut source.bytes, 3);
        free_page(&mut source.bytes, 11);
    }
    source
}

fn xml_data() -> Source {
    let original = canonical_variable_creation::source(canonical_variable_creation::Case::Empty);
    let mut bytes = original.bytes;
    let text = "<Event/>"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let binary = b"<BeforeChange/>";
    assert_eq!(text.len(), 16);
    assert_eq!(binary.len(), 15);
    let mut row = vec![5, 0, 7, 0, 0, 0];
    row.extend_from_slice(&text);
    row.extend_from_slice(binary);
    assert_eq!(row.len(), 37);
    for offset in [37_u16, 37, 22, 6, 3] {
        row.extend_from_slice(&offset.to_le_bytes());
    }
    row.push(0x17);
    assert_eq!(row.len(), 48);
    bytes[8 * PAGE..9 * PAGE].copy_from_slice(&packed(7, &[(0, row)]));
    put32(&mut bytes, 7 * PAGE + 16, 1);
    Source {
        bytes,
        tables: vec![Table {
            name: "Items".into(),
            columns: original
                .columns
                .iter()
                .map(|column| column.name.into())
                .collect(),
            rows: vec![vec![
                Value::Int32(7),
                Value::Text("<Event/>".into()),
                Value::Binary(binary.to_vec()),
                Value::Null,
                Value::Boolean(true),
            ]],
            definition: 7,
            data: vec![8],
        }],
        ordinary: true,
    }
}

/// Author one source from qualified independent originals, never a producer.
pub fn source(case: Case) -> Source {
    match case {
        Case::Extrema => return fixed(canonical_creation::Case::Extrema),
        Case::Named => return fixed(canonical_creation::Case::Named),
        Case::Multiple => return multiple(canonical_multiple_creation::Case::Populated),
        Case::Sixteen => return multiple(canonical_multiple_creation::Case::Sixteen),
        Case::RelocatedUser => return relocated(false),
        Case::RelocatedMaps => return relocated(true),
        Case::XmlData => return xml_data(),
        Case::InactiveXml => {
            let original =
                canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
            let mut bytes = original.bytes;
            let (start, end) = row_bounds(&bytes, 8, 0);
            assert_eq!(end - start, 63);
            bytes[start + 9..start + 17].copy_from_slice(b"<Event/>");
            bytes[end - 2] &= !0x10;
            let mut rows = original.rows;
            rows[0][4] = Value::Null;
            return Source {
                bytes,
                tables: vec![Table {
                    name: "Items".into(),
                    columns: original
                        .columns
                        .iter()
                        .map(|column| column.name.into())
                        .collect(),
                    rows,
                    definition: 7,
                    data: vec![8],
                }],
                ordinary: true,
            };
        }
        Case::PositiveProgram => {
            let original =
                canonical_before_change::source(5, canonical_before_change::Recipe::Sequential);
            let mut source = fixed(canonical_creation::Case::One);
            source.bytes = original.before;
            source.tables[0].rows = vec![
                vec![Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ];
            source.ordinary = false;
            return source;
        }
        _ => {}
    }
    let mut source = fixed(canonical_creation::Case::One);
    let positive = matches!(
        case,
        Case::ProgramLikeName | Case::ReorderedCatalog | Case::SplitData | Case::TrailingFreePage
    );
    match case {
        Case::ProgramLikeName => {
            let mut rows = records(&source.bytes, CATALOG_PAGE);
            rows[7] = catalog_row(7, "AutoExec");
            replace_catalog(
                &mut source,
                rows.into_iter().map(|row| (0, row)).collect(),
                8,
            );
            source.tables[0].name = "AutoExec".into();
        }
        Case::ReorderedCatalog => {
            let mut rows = records(&source.bytes, CATALOG_PAGE);
            rows.reverse();
            replace_catalog(
                &mut source,
                rows.into_iter().map(|row| (0, row)).collect(),
                8,
            );
        }
        Case::SplitData => {
            source = fixed(canonical_creation::Case::Extrema);
            let rows = records(&source.bytes, 8);
            assert_eq!(rows.len(), 2);
            source.bytes.resize(10 * PAGE, 0);
            source.bytes[8 * PAGE..9 * PAGE].copy_from_slice(&packed(7, &[(0, rows[0].clone())]));
            source.bytes[9 * PAGE..10 * PAGE].copy_from_slice(&packed(7, &[(0, rows[1].clone())]));
            map_members(&mut source.bytes, 3, 3, &[8, 9]);
            map_members(&mut source.bytes, 3, 7, &[8, 9]);
            source.tables[0].data = vec![8, 9];
        }
        Case::TrailingFreePage | Case::OrphanZeroPage | Case::OrphanPayloadPage => {
            source.bytes.resize(10 * PAGE, 0);
            if case == Case::TrailingFreePage {
                free_page(&mut source.bytes, 9);
            } else if case == Case::OrphanPayloadPage {
                source.bytes[9 * PAGE + 100..9 * PAGE + 115].copy_from_slice(b"<BeforeChange/>");
            }
        }
        Case::NonzeroFreePage => {
            source = relocated(false);
            source.bytes[7 * PAGE + 100] = 1;
        }
        Case::OpaqueExtra | Case::PresentEmptyExtra => {
            let payload = if case == Case::PresentEmptyExtra {
                Vec::new()
            } else {
                wide_properties::bag(&[
                    wide_properties::names(&["UnresolvedProgram"]),
                    wide_properties::group(
                        0x7777,
                        &[wide_properties::property(
                            0,
                            1,
                            12,
                            &wide_properties::text("opaque"),
                        )],
                    ),
                ])
            };
            let mut rows = records(&source.bytes, 4);
            rows[7] = extra_row(&rows[7], &payload);
            replace_catalog(
                &mut source,
                rows.into_iter().map(|row| (0, row)).collect(),
                8,
            );
        }
        Case::DeletedCatalogSlot | Case::Low24DefinitionAlias => {
            let mut rows = records(&source.bytes, 4)
                .into_iter()
                .map(|row| (0, row))
                .collect::<Vec<_>>();
            let deleted = case == Case::DeletedCatalogSlot;
            rows.push((
                if deleted { 0x8000 } else { 0 },
                catalog_row(if deleted { 11 } else { 0x0100_0007 }, "Alias"),
            ));
            replace_catalog(&mut source, rows, if deleted { 8 } else { 9 });
            if !deleted {
                // The extra LIVE catalog object is visible, even though its
                // aliased physical definition cannot qualify ordinary closure.
                source.tables.push(Table {
                    name: "Alias".into(),
                    columns: source.tables[0].columns.clone(),
                    rows: source.tables[0].rows.clone(),
                    definition: 7,
                    data: vec![8],
                });
            }
        }
        Case::MapAlias => put32(&mut source.bytes, 7 * PAGE + 59, (3 << 8) | 3),
        Case::SharedSystemMap => put32(&mut source.bytes, 6 * PAGE + 55, (3 << 8) | 1),
        Case::ShortCatalogRow | Case::ShortCatalogFooter => {
            let (start, end) = row_bounds(&source.bytes, 4, 7);
            if case == Case::ShortCatalogRow {
                put16(&mut source.bytes, start, 14);
            } else {
                put16(&mut source.bytes, end - 4, 8);
            }
        }
        Case::MissingLvalPairs | Case::DuplicateLvalMap => {
            let definition = 2 * PAGE;
            let length = u32::from_le_bytes(
                source.bytes[definition + 8..definition + 12]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let tail = definition + length - 42;
            assert_eq!(&source.bytes[tail..tail + 2], &5_u16.to_le_bytes());
            if case == Case::MissingLvalPairs {
                source.bytes[tail..definition + PAGE].fill(0);
                put16(&mut source.bytes, tail, 0xffff);
                put32(&mut source.bytes, definition + 8, (length - 40) as u32);
                put16(
                    &mut source.bytes,
                    definition + 2,
                    (PAGE - (length - 40) - 8) as u16,
                );
            } else {
                put32(&mut source.bytes, tail + 12, (3 << 8) | 8);
            }
        }
        Case::UnknownHeaderByte => source.bytes[200] = 1,
        Case::UnknownFreeByte => source.bytes[8 * PAGE + 100] = 1,
        Case::ExtraEofByte => source.bytes.push(0),
        _ => unreachable!("source case handled above"),
    }
    source.ordinary = positive;
    source
}
