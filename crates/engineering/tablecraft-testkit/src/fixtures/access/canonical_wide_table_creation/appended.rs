//! Fresh wide table pages beside exact retained storage and property history.
use super::{PAGE, PROPERTY_CHUNK, Source, definition_pages, logical_definition, properties, word};
use crate::fixtures::access::{
    canonical_created_default_zero, canonical_creation, canonical_creation_properties,
    canonical_native17_creation, canonical_retained_add_table::extend_global_map,
    packed_record::append_record, put32,
};

fn allocate_map(after: &mut [u8], member: usize) {
    let mut row = [0; 13];
    let base = member / 64 * 64;
    put32(&mut row, 1, base as u32);
    row[5 + (member - base) / 8] |= 1 << ((member - base) % 8);
    append_record(&mut after[3 * PAGE..4 * PAGE], &row);
}

fn add_property_page(after: &mut [u8], slot: usize, member: usize) {
    let page = &mut after[3 * PAGE..4 * PAGE];
    let start = word(page, 14 + 2 * slot);
    let end = word(page, 14 + 2 * (slot - 1));
    assert_eq!(end - start, 13);
    assert_eq!(
        page[start], 0,
        "this raw source retains inline catalog maps"
    );
    let base = u32::from_le_bytes(page[start + 1..start + 5].try_into().unwrap()) as usize;
    assert!((base..base + 64).contains(&member));
    page[start + 5 + (member - base) / 8] |= 1 << ((member - base) % 8);
}

/// Append the same exact ID/Field1..254 declaration at real EOF. Older bodies,
/// directory slots, retired pages, existing tables and hint bits stay retained.
pub fn source(before: &[u8], table: &str) -> Source {
    assert_eq!(before.len() % PAGE, 0);
    let root = before.len() / PAGE;
    let data = root + 3;
    let index = data + 1;
    let property_first = index + 1;
    let map_count = word(&before[3 * PAGE..4 * PAGE], 12);
    assert!(map_count + 2 < 256);
    let seed =
        canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly);
    let logical = logical_definition(
        &seed,
        (3 << 8) | map_count as u32,
        (3 << 8) | (map_count + 1) as u32,
        (3 << 8) | (map_count + 2) as u32,
        index as u32,
    );
    let definitions = definition_pages(&logical, root);
    let body = properties();
    let chunks = body.chunks(PROPERTY_CHUNK).collect::<Vec<_>>();
    // NO_CLONE: AFTER independently changes catalog/maps and adds storage while
    // callers retain the exact immutable BEFORE for whole-image preservation.
    let mut after = before.to_vec();
    for member in [data, data, index] {
        allocate_map(&mut after, member);
    }
    for page in property_first..property_first + chunks.len() {
        add_property_page(&mut after, 16, page);
    }
    let last = property_first + chunks.len() - 1;
    assert!(chunks.last().unwrap().len() < PROPERTY_CHUNK);
    add_property_page(&mut after, 17, last);
    let row = canonical_creation::catalog_record(root as u32, 0x0f00_0001, table, 1, 0);
    let row = canonical_native17_creation::catalog_row(&row);
    let mut row = canonical_creation_properties::attach(&row, &[]);
    let descriptor = row.len() - 29 - 12;
    put32(&mut row, descriptor, body.len() as u32);
    put32(&mut row, descriptor + 4, (property_first as u32) << 8);
    append_record(&mut after[4 * PAGE..5 * PAGE], &row);
    let count = u32::from_le_bytes(before[2 * PAGE + 16..2 * PAGE + 20].try_into().unwrap());
    put32(&mut after, 2 * PAGE + 16, count + 1);
    for page in definitions {
        after.extend(page);
    }
    after.extend(canonical_native17_creation::packed(root as u32, Vec::new()));
    // NO_CLONE: This appended table owns an independently mutable leaf with a
    // fresh owner; the original raw seed must retain its own index identity.
    let mut leaf = seed[9 * PAGE..10 * PAGE].to_vec();
    put32(&mut leaf, 4, root as u32);
    after.extend(leaf);
    for (index, chunk) in chunks.iter().enumerate() {
        let next = if index + 1 < chunks.len() {
            ((property_first + index + 1) as u32) << 8
        } else {
            0
        };
        let mut row = next.to_le_bytes().to_vec();
        row.extend_from_slice(chunk);
        after.extend(canonical_native17_creation::packed(
            u32::from_le_bytes(*b"LVAL"),
            vec![row],
        ));
    }
    extend_global_map(&mut after, root);
    assert_eq!(&after[7 * PAGE..before.len()], &before[7 * PAGE..]);
    Source {
        // NO_CLONE: The fixture exposes complete independent BEFORE and AFTER
        // images for consuming native preservation assertions.
        before: before.to_vec(),
        after,
        definition_pages: (root..root + 3).map(|page| page as u32).collect(),
        data_page: data as u32,
        index_page: index as u32,
        property_pages: (property_first..property_first + chunks.len())
            .map(|page| page as u32)
            .collect(),
        property_body: body,
    }
}
