//! Complete literal TD tails for the allocation-qualified relationship source.
//!
//! The older read/delete donor intentionally left zero-filled trailing bytes.
//! Keep that donor unchanged: this source explicitly declares the end of every
//! map-declaration list. Catalog Lv fields are absent/NULL, not invented zero
//! pairs or new ownership. User fields, relationships, indexes and ROWs stay exact.
use super::{model, support};

const PAGE: usize = 4096;
const FREE: usize = 2;
const NEXT: usize = 4;
const LENGTH: usize = 8;
const HEADER_BYTES: usize = 8;
const FIELD_COUNT: usize = 45;
const LOGICAL_COUNT: usize = 47;
const PHYSICAL_COUNT: usize = 51;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const HISTORY_BYTES: usize = 12;
const PHYSICAL_BYTES: usize = 52;
const LOGICAL_BYTES: usize = 28;
const NAME_LENGTH_BYTES: usize = 2;
const OWNED_REFERENCE: usize = 55;
const CANDIDATE_REFERENCE: usize = 59;
const DATA_HEADER: usize = 14;
const DATA_SLOTS: usize = 12;
const MAP_BYTES: usize = 69;
const MAP_PREFIX: usize = 5;

pub(super) fn source(code: u8) -> model::Model {
    let mut source = model::database(code, false, true, false);
    let original = source.bytes.clone();
    let mut modified = Vec::new();
    for number in [2, 7, 10, 15, 20] {
        let base = number * PAGE;
        let page = &mut source.bytes[base..base + PAGE];
        assert_eq!(&page[..2], &[2, 1]);
        assert_eq!(number32(page, NEXT), 0);
        assert_eq!(number32(page, LENGTH), PAGE);
        let fields = number16(page, FIELD_COUNT);
        let physical = number32(page, PHYSICAL_COUNT);
        let logical = number32(page, LOGICAL_COUNT);
        let mut terminal = FIELD_START + physical * HISTORY_BYTES + fields * FIELD_BYTES;
        for _ in 0..fields {
            terminal = after_name(page, terminal);
        }
        terminal += physical * PHYSICAL_BYTES + logical * LOGICAL_BYTES;
        for _ in 0..logical {
            terminal = after_name(page, terminal);
        }
        assert!(page[terminal..].iter().all(|byte| *byte == 0));
        let length = terminal + 2;
        assert!(length + HEADER_BYTES <= PAGE);
        support::put16(page, terminal, 0xffff);
        support::put32(page, LENGTH, length as u32);
        support::put16(page, FREE, (PAGE - length - HEADER_BYTES) as u16);
        modified.push(base + FREE..base + FREE + 2);
        modified.push(base + LENGTH..base + LENGTH + 4);
        modified.push(base + terminal..base + length);
    }
    for table in 0..3 {
        let definition = model::definition(table);
        let base = definition * PAGE;
        let reference = number32(&source.bytes, base + OWNED_REFERENCE);
        assert_eq!(reference, (definition + 1) << 8);
        assert_eq!(number32(&source.bytes, base + CANDIDATE_REFERENCE), 0);
        let storage = (reference >> 8) * PAGE;
        assert_eq!(number16(&source.bytes, storage + DATA_SLOTS), 1);
        let original_start = number16(&source.bytes, storage + DATA_HEADER) & 0x1fff;
        let original_map = &source.bytes[storage + original_start..storage + PAGE];
        assert_eq!(original_map.len(), 9);
        assert_eq!(&original_map[..MAP_PREFIX], &[0; MAP_PREFIX]);
        let mut owned = vec![0; MAP_BYTES];
        owned[..original_map.len()].copy_from_slice(original_map);

        let data = model::data(table);
        let records = &source.records[table];
        let data_base = data * PAGE;
        let page = &source.bytes[data_base..data_base + PAGE];
        assert_eq!(number16(page, DATA_SLOTS), records.len());
        let mut end = PAGE;
        for (slot, record) in records.iter().enumerate() {
            let word = number16(page, DATA_HEADER + slot * 2);
            assert_eq!(word & 0xe000, 0);
            let start = word & 0x1fff;
            assert_eq!(&page[start..end], record);
            end = start;
        }
        let free = end - (DATA_HEADER + records.len() * 2);
        assert_eq!(
            free,
            PAGE - DATA_HEADER - records.len() * 2 - records.iter().map(Vec::len).sum::<usize>()
        );
        assert_eq!(number16(page, FREE), 0);
        assert!(free >= records.iter().map(Vec::len).max().unwrap() + 2);
        support::put16(&mut source.bytes, data_base + FREE, free as u16);
        modified.push(data_base + FREE..data_base + FREE + 2);
        let mut candidate = vec![0; MAP_BYTES];
        candidate[MAP_PREFIX + data / 8] |= 1 << (data % 8);
        assert_eq!(
            owned[MAP_PREFIX + data / 8] & (1 << (data % 8)),
            1 << (data % 8)
        );
        let mut maps = support::data_page(0, &[(0, owned), (0, candidate)]);
        assert_eq!(number16(&maps, DATA_SLOTS), 2);
        assert_eq!(number16(&maps, FREE), 0);
        let first = number16(&maps, DATA_HEADER);
        let last = number16(&maps, DATA_HEADER + 2);
        assert_eq!(PAGE - first, MAP_BYTES);
        assert_eq!(first - last, MAP_BYTES);
        let map_free = last - (DATA_HEADER + 2 * 2);
        assert_eq!(map_free, 3940);
        support::put16(&mut maps, FREE, map_free as u16);
        assert_eq!(number16(&maps, FREE), 3940);
        source.bytes[storage..storage + PAGE].copy_from_slice(&maps);
        support::put32(
            &mut source.bytes,
            base + CANDIDATE_REFERENCE,
            reference as u32 | 1,
        );
        modified.push(storage..storage + PAGE);
        modified.push(base + CANDIDATE_REFERENCE..base + CANDIDATE_REFERENCE + 4);
    }
    assert_eq!(source.bytes.len(), original.len());
    for (offset, (before, after)) in original.iter().zip(&source.bytes).enumerate() {
        if before != after {
            assert!(modified.iter().any(|range| range.contains(&offset)));
        }
    }
    source
}

fn number16(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize
}

fn number32(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn after_name(page: &[u8], position: usize) -> usize {
    let bytes = number16(page, position);
    assert!(bytes.is_multiple_of(2));
    let end = position + NAME_LENGTH_BYTES + bytes;
    assert!(end <= PAGE);
    end
}
