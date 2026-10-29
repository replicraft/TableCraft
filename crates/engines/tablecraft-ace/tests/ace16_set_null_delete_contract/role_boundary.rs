//! Real nullable alternate parent keys bind declared UPDATE actions explicitly.
use super::{model, support};
use std::io::Cursor;
use tablecraft_ace::{RelationshipCheckStatus, check_table, check_table_constraints};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const LOGICAL_BYTES: usize = 28;
const PHYSICAL_BYTES: usize = 52;
const PHYSICAL_FLAGS: usize = 46;
const FOREIGN_ROLE: usize = 12;
const FOREIGN_INDEX: usize = 13;
const FOREIGN_TABLE: usize = 17;
const UPDATE_ACTION: usize = 21;
const INDEX_KIND: usize = 23;
const ENTRY_START: usize = 480;
const BITMAP: usize = 27;

pub(super) fn source(code: u8) -> model::Model {
    let mut source = model::source(code, false, false);

    // Parents.Score --DELETE SET NULL--> Items.Score is retained. Items.Score
    // now also parents Other.Score through a separate UPDATE CASCADE relation.
    // Its nullable UNIQUE key is deliberately not a required primary key.
    let physical = model::definition(0) * PAGE_BYTES + source.logical_start - PHYSICAL_BYTES;
    source.bytes[physical + PHYSICAL_FLAGS] = 0x81;
    let scores = [Some(10_i32), Some(20), None, Some(30)];
    for (row, score) in scores.into_iter().enumerate() {
        source.rows[0][row][1] = score.map_or(Value::Null, Value::Int32);
        source.records[0][row][6..10].copy_from_slice(&score.unwrap_or(0).to_le_bytes());
        if score.is_some() {
            source.records[0][row][14] |= 2;
        } else {
            source.records[0][row][14] &= !2;
        }
    }
    let records: Vec<_> = source.records[0]
        .iter()
        .cloned()
        .map(|record| (0, record))
        .collect();
    let data = model::data(0) * PAGE_BYTES;
    source.bytes[data..data + PAGE_BYTES]
        .copy_from_slice(&support::data_page(model::definition(0) as u32, &records));
    write_items_index(&mut source, &scores);

    // The previous second parent link becomes an ordinary alias. The matching
    // incoming link on Other now points to Items' outgoing logical index 1.
    let old_parent = model::definition(1) * PAGE_BYTES + source.logical_start + 2 * LOGICAL_BYTES;
    source.bytes[old_parent + FOREIGN_ROLE..old_parent + INDEX_KIND + 1].fill(0);
    link(&mut source, 0, 1, 1, 2, 0);
    link(&mut source, 2, 0, 2, 0, 1);
    ordinary_unique_alias(&mut source);

    let relationships = vec![
        relationship("First", "Parents", "Items", 0x2000),
        relationship("Second", "Items", "Other", 0x100),
    ];
    support::put32(&mut source.bytes, 7 * PAGE_BYTES + 16, 2);
    source.bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&support::data_page(
        7,
        &relationships
            .into_iter()
            .map(|row| (0, row))
            .collect::<Vec<_>>(),
    ));
    source
}

fn ordinary_unique_alias(source: &mut model::Model) {
    // Raw relationship inspection deliberately does not qualify foreign index
    // declarations. A real ordinary alias declares this alternate UNIQUE parent
    // key independently of the two reciprocal relationship aliases.
    let definition = model::definition(0) * PAGE_BYTES;
    support::put32(&mut source.bytes, definition + 47, 3);
    let alias = definition + source.logical_start + 2 * LOGICAL_BYTES;
    source.bytes[alias..alias + LOGICAL_BYTES].fill(0);
    support::put32(&mut source.bytes, alias + 4, 2);
    let mut names = definition + source.logical_start + 3 * LOGICAL_BYTES;
    for name in ["Incoming", "Outgoing", "NullableUnique"] {
        let encoded: Vec<_> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        support::put16(&mut source.bytes, names, encoded.len() as u16);
        names += 2;
        source.bytes[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
}

fn link(
    source: &mut model::Model,
    target: usize,
    logical: usize,
    role: u8,
    peer: usize,
    peer_logical: u32,
) {
    let at =
        model::definition(target) * PAGE_BYTES + source.logical_start + logical * LOGICAL_BYTES;
    source.bytes[at + FOREIGN_ROLE..at + INDEX_KIND + 1].fill(0);
    source.bytes[at + FOREIGN_ROLE] = role;
    support::put32(&mut source.bytes, at + FOREIGN_INDEX, peer_logical);
    support::put32(
        &mut source.bytes,
        at + FOREIGN_TABLE,
        model::definition(peer) as u32,
    );
    source.bytes[at + UPDATE_ACTION] = 1;
    source.bytes[at + INDEX_KIND] = 2;
}

fn write_items_index(source: &mut model::Model, scores: &[Option<i32>; 4]) {
    let mut page = vec![0; PAGE_BYTES];
    page[..2].copy_from_slice(&[4, 1]);
    support::put32(&mut page, 4, model::definition(0) as u32);
    let mut entries = Vec::new();
    for (row, score) in scores.iter().enumerate() {
        let mut entry = vec![if score.is_some() { 127 } else { 0 }];
        if let Some(score) = score {
            entry.extend(((*score as u32) ^ 0x80000000).to_be_bytes());
        }
        entry.extend((((model::data(0) as u32) << 8) | row as u32).to_be_bytes());
        entries.push(entry);
    }
    entries.sort();
    let mut end = 0;
    for entry in entries {
        page[ENTRY_START + end..ENTRY_START + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        page[BITMAP + end / 8] |= 1 << (end % 8);
    }
    support::put16(&mut page, 2, (PAGE_BYTES - ENTRY_START - end) as u16);
    let root = model::root(0) * PAGE_BYTES;
    source.bytes[root..root + PAGE_BYTES].copy_from_slice(&page);
}

fn relationship(name: &str, parent: &str, child: &str, flags: u32) -> Vec<u8> {
    let mut row = 8_u16.to_le_bytes().to_vec();
    for value in [flags, 1, 0] {
        row.extend(value.to_le_bytes());
    }
    let mut offsets = vec![row.len() as u16];
    for text in [name, parent, child, "Score", "Score"] {
        row.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(row.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(5_u16.to_le_bytes());
    row.push(255);
    row
}

#[test]
fn alternate_parent_sources_qualify_before_invoking_the_producer() {
    for code in [5, 6] {
        let source = source(code);
        model::before(&source);
        let checked = check_table(
            &mut Cursor::new(&source.bytes),
            CatalogScope::new(1),
            "Items",
            &mut model::budget(),
            &|| false,
        )
        .unwrap();
        assert!(checked.indexes().iter().any(|index| {
            index
                .index()
                .constraints()
                .is_some_and(|rules| rules.unique() && !rules.required() && !rules.primary_key())
        }));
        assert_eq!(source.rows[0][2][1], Value::Null);
        for target in 0..3 {
            let report = check_table_constraints(
                &mut Cursor::new(&source.bytes),
                CatalogScope::new(1),
                model::table(target),
                &mut model::budget(),
                &|| false,
            )
            .unwrap();
            for relationship in report.relationships() {
                assert_eq!(
                    relationship.status(),
                    RelationshipCheckStatus::Checked,
                    "table {} relationship {}",
                    model::table(target),
                    relationship.relationship().declaration().name().as_str(),
                );
            }
        }
    }
}

#[test]
fn only_effective_surviving_parent_key_changes_require_update_action_support() {
    for code in [5, 6] {
        let source = source(code);
        model::before(&source);
        for seeds in [vec![], vec![(1, &[][..])], vec![(0, &[][..])]] {
            let candidate = model::run(&source, &seeds)
                .unwrap_or_else(|error| panic!("header {code}, seeds {seeds:?}: {error:?}"));
            assert_eq!(candidate.deleted_rows(), 0);
            assert_eq!(candidate.before_bytes(), source.bytes);
            assert_eq!(candidate.candidate_bytes(), source.bytes);
        }

        // Deleting the child itself leaves no surviving parent-key assignment.
        let expected = model::expected(&source, [&[0], &[0], &[]], &[]);
        model::qualify(&expected);
        let candidate = model::run(&source, &[(1, &[0]), (0, &[0])]).unwrap();
        model::verify(&source, &expected, &candidate);

        // Key 10 has no downstream child. Its effective NULL assignment still
        // binds the declared UPDATE CASCADE graph without changing Other.
        let expected = model::expected(&source, [&[], &[0], &[]], &[0]);
        model::qualify(&expected);
        let candidate = model::run(&source, &[(1, &[0])]).unwrap();
        model::verify(&source, &expected, &candidate);
    }
}
