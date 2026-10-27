//! Independent literal declaration/row model over unchanged authored page helpers.
use super::{restrict_model, support};
use std::io::Cursor;
use tablecraft_ace::{
    Ace16DeletePreview, EditError, IndexCheckStatus, RowDelete, TableRowDeletes, check_table,
    check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub use restrict_model::{Model, budget, capture, data, definition, requests, root, table};

pub const PAGE_BYTES: usize = 4096;
pub const LOGICAL_BYTES: usize = 28;
pub const INDEX_KIND: usize = 23;
pub const DELETE_ACTION: usize = 22;
pub const FOREIGN_START: usize = 12;
const PHYSICAL_BYTES: usize = 52;
const PHYSICAL_FLAGS: usize = 46;
const TABLE_ROWS: usize = 16;
const SLOT_START: usize = 14;
const LEAF_ENTRIES: usize = 480;
const LEAF_BITMAP: usize = 27;

pub fn database(code: u8, second_cascade: bool) -> Model {
    let mut model = restrict_model::database(code, false, true, false);
    primary(&mut model, 1);
    rules(
        &mut model,
        0x1000,
        if second_cascade { 0x1000 } else { 0 },
        1,
        u8::from(second_cascade),
    );
    model
}

pub fn primary(model: &mut Model, index: usize) {
    let definition = definition(index) * PAGE_BYTES;
    let physical = model.logical_start - PHYSICAL_BYTES;
    model.bytes[definition + physical + PHYSICAL_FLAGS] = 0x89;
    model.bytes[definition + model.logical_start + INDEX_KIND] = 1;
}

pub fn rules(
    model: &mut Model,
    first_flags: u32,
    second_flags: u32,
    first_action: u8,
    second_action: u8,
) {
    let records = vec![
        relationship("First", "Parents", "Items", first_flags),
        relationship("Second", "Parents", "Other", second_flags),
    ];
    catalog_relationships(model, records);
    for (child, logical, action) in [(0, 1, first_action), (2, 2, second_action)] {
        model.bytes[definition(child) * PAGE_BYTES + model.logical_start + DELETE_ACTION] = action;
        model.bytes[definition(1) * PAGE_BYTES
            + model.logical_start
            + logical * LOGICAL_BYTES
            + DELETE_ACTION] = action;
    }
}

fn catalog_relationships(model: &mut Model, records: Vec<Vec<u8>>) {
    support::put32(
        &mut model.bytes,
        7 * PAGE_BYTES + TABLE_ROWS,
        records.len() as u32,
    );
    let records: Vec<_> = records.into_iter().map(|row| (0, row)).collect();
    model.bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&support::data_page(7, &records));
}

fn relationship(name: &str, parent: &str, child: &str, flags: u32) -> Vec<u8> {
    let mut row = 8_u16.to_le_bytes().to_vec();
    for value in [flags, 1, 0] {
        row.extend(value.to_le_bytes());
    }
    let mut offsets = vec![row.len() as u16];
    for value in [name, parent, child, "Score", "Score"] {
        row.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(row.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(5_u16.to_le_bytes());
    row.push(0xff);
    row
}

pub fn run(model: &Model, seeds: &[(usize, &[usize])]) -> Result<Ace16DeletePreview, EditError> {
    let owned: Vec<_> = seeds
        .iter()
        .map(|&(target, rows)| requests(model, target, rows))
        .collect();
    let groups: Vec<_> = seeds
        .iter()
        .zip(&owned)
        .map(|(&(target, _), rows)| TableRowDeletes::new(table(target), rows))
        .collect();
    capture(&model.bytes).preview_ace16_cascading_deletes(&groups, &mut budget(), &|| false)
}

pub fn before(model: &Model) {
    for target in 0..3 {
        let report = check_table(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            report
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(&model.bytes),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

pub fn require_primary(model: &Model, target: usize) {
    let checked = check_table(
        &mut Cursor::new(&model.bytes),
        CatalogScope::new(1),
        table(target),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(checked.indexes().iter().any(|checked| {
        let index = checked.index();
        index
            .constraints()
            .is_some_and(|rules| rules.primary_key() && rules.unique() && rules.required())
            && index.columns().len() == 1
            && index.columns()[0].number() == 1
    }));
}

pub fn verify(model: &Model, candidate: &Ace16DeletePreview, deleted: [&[usize]; 3]) {
    assert_eq!(candidate.before_bytes(), model.bytes);
    assert_eq!(
        candidate.deleted_rows(),
        deleted.iter().map(|rows| rows.len()).sum::<usize>()
    );
    let after = candidate.candidate_bytes();
    assert_eq!(after.len(), model.bytes.len());
    for target in 0..3 {
        let expected: Vec<_> = model.rows[target]
            .iter()
            .enumerate()
            .filter(|(row, _)| !deleted[target].contains(row))
            .map(|(_, row)| row.clone())
            .collect();
        let actual = read_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), expected);
        let checked = check_table(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        check_table_constraints(
            &mut Cursor::new(after),
            CatalogScope::new(1),
            table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();

        let start = definition(target) * PAGE_BYTES;
        assert_eq!(
            &after[start..start + TABLE_ROWS],
            &model.bytes[start..start + TABLE_ROWS]
        );
        assert_eq!(
            &after[start + TABLE_ROWS + 4..start + PAGE_BYTES],
            &model.bytes[start + TABLE_ROWS + 4..start + PAGE_BYTES]
        );
        assert_eq!(
            u32::from_le_bytes(
                after[start + TABLE_ROWS..start + TABLE_ROWS + 4]
                    .try_into()
                    .unwrap()
            ) as usize,
            model.rows[target].len() - deleted[target].len()
        );
        for row in 0..model.records[target].len() {
            let slot = data(target) * PAGE_BYTES + SLOT_START + row * 2;
            let old = u16::from_le_bytes(model.bytes[slot..slot + 2].try_into().unwrap());
            let expected = if deleted[target].contains(&row) {
                old | 0xc000
            } else {
                old
            };
            assert_eq!(&after[slot..slot + 2], &expected.to_le_bytes());
            let payload = data(target) * PAGE_BYTES + usize::from(old & 0x1fff);
            assert_eq!(
                &after[payload..payload + model.records[target][row].len()],
                model.records[target][row]
            );
        }
        if deleted[target].is_empty() {
            let start = root(target) * PAGE_BYTES;
            assert_eq!(
                &after[start..start + PAGE_BYTES],
                &model.bytes[start..start + PAGE_BYTES]
            );
        }
    }
    // Root-leaf removals retain every allocation map and inert catalog byte.
    for page in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 16, 18, 21, 23] {
        assert_eq!(
            &after[page * PAGE_BYTES..(page + 1) * PAGE_BYTES],
            &model.bytes[page * PAGE_BYTES..(page + 1) * PAGE_BYTES]
        );
    }
}

pub fn cycle() -> Model {
    let mut model = database(6, true);
    // Two genuine required primary populations with reciprocal Cascade edges.
    // Both sides have complete matching non-NULL keys; this is not a malformed
    // reverse edge attached to duplicate/nullable child keys.
    for target in [0, 1] {
        model.rows[target] = [10_i32, 20, 30]
            .into_iter()
            .enumerate()
            .map(|(row, key)| vec![Value::Int32(row as i32 + 1), Value::Int32(key), Value::Null])
            .collect();
        model.records[target] = [10_i32, 20, 30]
            .into_iter()
            .enumerate()
            .map(|(row, key)| {
                let mut payload = 3_u16.to_le_bytes().to_vec();
                payload.extend((row as i32 + 1).to_le_bytes());
                payload.extend(key.to_le_bytes());
                payload.extend(0_i32.to_le_bytes());
                payload.push(3);
                payload
            })
            .collect();
        let records: Vec<_> = model.records[target]
            .iter()
            .cloned()
            .map(|row| (0, row))
            .collect();
        model.bytes[data(target) * PAGE_BYTES..(data(target) + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(definition(target) as u32, &records));
        support::put32(
            &mut model.bytes,
            definition(target) * PAGE_BYTES + TABLE_ROWS,
            3,
        );
        primary(&mut model, target);
        logical_cycle(&mut model, target, 1 - target);
        let mut leaf = vec![0; PAGE_BYTES];
        leaf[..2].copy_from_slice(&[4, 1]);
        support::put32(&mut leaf, 4, definition(target) as u32);
        let mut end = 0;
        for (row, key) in [10_u32, 20, 30].into_iter().enumerate() {
            let mut entry = vec![127];
            entry.extend((key ^ 0x80000000).to_be_bytes());
            entry.extend((((data(target) as u32) << 8) | row as u32).to_be_bytes());
            leaf[LEAF_ENTRIES + end..LEAF_ENTRIES + end + entry.len()].copy_from_slice(&entry);
            end += entry.len();
            leaf[LEAF_BITMAP + end / 8] |= 1 << (end % 8);
        }
        support::put16(&mut leaf, 2, (PAGE_BYTES - LEAF_ENTRIES - end) as u16);
        model.bytes[root(target) * PAGE_BYTES..(root(target) + 1) * PAGE_BYTES]
            .copy_from_slice(&leaf);
    }
    let at = definition(2) * PAGE_BYTES + model.logical_start;
    model.bytes[at + FOREIGN_START..at + INDEX_KIND + 1].fill(0);
    catalog_relationships(
        &mut model,
        vec![
            relationship("Forward", "Items", "Parents", 0x1000),
            relationship("Reverse", "Parents", "Items", 0x1000),
        ],
    );
    model
}

fn logical_cycle(model: &mut Model, target: usize, other: usize) {
    let start = definition(target) * PAGE_BYTES;
    support::put32(&mut model.bytes, start + 47, 4);
    let mut names = start + model.logical_start + 4 * LOGICAL_BYTES;
    for (logical, name) in ["CyclePrimary", "Outgoing", "Incoming", "CycleAlias"]
        .into_iter()
        .enumerate()
    {
        let at = start + model.logical_start + logical * LOGICAL_BYTES;
        model.bytes[at..at + LOGICAL_BYTES].fill(0);
        support::put32(&mut model.bytes, at + 4, logical as u32);
        model.bytes[at + INDEX_KIND] = if logical == 0 {
            1
        } else if logical == 3 {
            0
        } else {
            2
        };
        if logical == 1 || logical == 2 {
            model.bytes[at + FOREIGN_START] = logical as u8;
            support::put32(&mut model.bytes, at + 13, if logical == 1 { 2 } else { 1 });
            support::put32(&mut model.bytes, at + 17, definition(other) as u32);
            model.bytes[at + DELETE_ACTION] = 1;
        }
        let encoded: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        support::put16(&mut model.bytes, names, encoded.len() as u16);
        names += 2;
        model.bytes[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
}

pub fn stale(model: &Model, target: usize, row: usize) -> RowDelete {
    let mut bytes = model.records[target][row].clone();
    bytes[2] ^= 1;
    RowDelete::new(row, bytes)
}
