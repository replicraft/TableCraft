//! Complete exclusive index-footprint retirement, without truncating the file.
//! This is a checked candidate policy oracle, not an Access application byte claim.
use super::Source;
use crate::fixtures::access::{put16, put32};

const PAGE: usize = 4096;
fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}
fn long(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

/// Remove the single logical declaration and its exclusive physical tree/map.
/// Root and all descendants plus the dedicated map page are zero/free; every
/// unrelated byte and the original grown file length are preserved.
pub fn after(source: &Source) -> Vec<u8> {
    from_indexed(source, &source.expected)
}

fn from_indexed(source: &Source, before: &[u8]) -> Vec<u8> {
    from_indexed_with_properties(source, before, super::definition::Properties::Ordinary)
}

pub(super) fn from_unique_indexed(source: &Source, before: &[u8]) -> Vec<u8> {
    from_indexed_with_properties(source, before, super::definition::Properties::Unique)
}

fn from_indexed_with_properties(
    source: &Source,
    before: &[u8],
    properties: super::definition::Properties,
) -> Vec<u8> {
    let base = source.target_definition as usize * PAGE;
    let (declaration, retire) = definition(&before[base..base + PAGE], source, properties);
    // NO_CLONE: Complete independent AFTER coexists with its indexed SOURCE.
    let mut after = before.to_vec();
    after[base..base + PAGE].copy_from_slice(&declaration);
    if !retire {
        return after;
    }
    assert!(source.index_pages.contains(&source.root));
    for page in &source.index_pages {
        let start = *page as usize * PAGE;
        assert!(matches!(before[start], 3 | 4));
        assert_eq!(
            &before[start + 4..start + 8],
            &source.target_definition.to_le_bytes()
        );
        after[start..start + PAGE].fill(0);
        free(&mut after, *page);
    }
    let host = source.map_host as usize * PAGE;
    assert_eq!(&before[host..host + 2], &[1, 1]);
    assert_eq!(&before[host + 4..host + 8], &[0; 4]);
    assert_eq!(word(before, host + 12), 1);
    let start = host + word(before, host + 14);
    assert_eq!(before[start], 0);
    let first = *source.index_pages.first().unwrap();
    assert_eq!(long(before, start + 1), first as usize);
    for (byte_index, byte) in before[start + 5..host + PAGE].iter().enumerate() {
        for bit in 0..8 {
            let page = first + (byte_index * 8 + bit) as u32;
            assert_eq!(*byte & (1 << bit) != 0, source.index_pages.contains(&page));
        }
    }
    assert!(!source.index_pages.contains(&source.map_host));
    after[host..host + PAGE].fill(0);
    free(&mut after, source.map_host);
    assert_eq!(after.len(), before.len());
    after
}

fn free(after: &mut [u8], page: u32) {
    let global = PAGE;
    assert_eq!(&after[global..global + 2], &[1, 1]);
    assert_eq!(word(after, global + 12), 1);
    let start = global + word(after, global + 14);
    assert_eq!(&after[start..start + 5], &[0; 5]);
    let cell = start + 5 + page as usize / 8;
    assert!(cell < 2 * PAGE);
    let bit = 1 << (page % 8);
    assert_eq!(
        after[cell] & bit,
        0,
        "original footprint member must be allocated"
    );
    after[cell] |= bit;
}

fn definition(
    original: &[u8],
    source: &Source,
    properties: super::definition::Properties,
) -> (Vec<u8>, bool) {
    let logical = long(original, 47);
    let physical = long(original, 51);
    assert!(logical > 0 && physical > 0);
    let columns = 63 + physical * 12;
    let mut physical_start = columns + word(original, 45) * 25;
    for _ in 0..word(original, 45) {
        physical_start += 2 + word(original, physical_start);
    }
    let logical_start = physical_start + physical * 52;
    let names_start = logical_start + logical * 28;
    let length = long(original, 8);
    let mut names = Vec::new();
    let mut at = names_start;
    let mut selected = None;
    for position in 0..logical {
        let end = at + 2 + word(original, at);
        assert!(end <= length);
        let units = original[at + 2..end]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        let name = String::from_utf16(&units).unwrap();
        if name == source.index_name {
            assert!(selected.replace(position).is_none());
        }
        names.push(at..end);
        at = end;
    }
    assert_eq!(at + 2, length);
    assert_eq!(&original[at..length], &[255, 255]);
    let selected = selected.unwrap();
    let selected_logical = logical_start + selected * 28;
    assert_eq!(long(original, selected_logical), 1625);
    let removed = long(original, selected_logical + 8);
    assert!(removed < physical);
    let selected_physical = physical_start + removed * 52;
    assert_eq!(long(original, selected_physical), 1923);
    assert_eq!(
        long(original, selected_physical + 34),
        (source.map_host << 8) as usize
    );
    assert_eq!(long(original, selected_physical + 38), source.root as usize);
    assert_eq!(
        original[selected_physical + 46],
        properties.flags(),
        "exact selected physical index policy"
    );
    let aliases = (0..logical)
        .filter(|position| long(original, logical_start + position * 28 + 8) == removed)
        .count();
    let retire = aliases == 1;
    let mut frame;
    if retire {
        frame = original[..63 + removed * 12].to_vec();
        frame.extend_from_slice(&original[63 + (removed + 1) * 12..columns]);
        frame.extend_from_slice(&original[columns..physical_start + removed * 52]);
        frame.extend_from_slice(&original[physical_start + (removed + 1) * 52..logical_start]);
    } else {
        frame = original[..logical_start].to_vec();
    }
    for position in 0..logical {
        if position == selected {
            continue;
        }
        let mut alias =
            original[logical_start + position * 28..logical_start + (position + 1) * 28].to_vec();
        let reference = long(&alias, 8);
        if retire && reference > removed {
            put32(&mut alias, 8, (reference - 1) as u32);
        }
        // Logical creation identity at+4 stays exact, independent of position.
        frame.extend(alias);
    }
    for (position, name) in names.into_iter().enumerate() {
        if position != selected {
            frame.extend_from_slice(&original[name]);
        }
    }
    frame.extend([255, 255]);
    let new_length = frame.len();
    assert!(new_length < length);
    put32(&mut frame, 8, new_length as u32);
    put16(&mut frame, 2, (PAGE - new_length - 8) as u16);
    put32(&mut frame, 47, (logical - 1) as u32);
    put32(&mut frame, 51, (physical - usize::from(retire)) as u32);
    // NO_CLONE: Preserve the existing opaque tail after the old active length;
    // only the explicitly removed active suffix is cleared in this new page.
    let mut result = original.to_vec();
    result[..new_length].copy_from_slice(&frame);
    result[new_length..length].fill(0);
    (result, retire)
}

/// A genuine shared physical tree with one independently authored logical alias.
pub struct AliasCase {
    /// Complete indexed source with both logical declarations.
    pub before: Vec<u8>,
    /// Removing ScoreLookup keeps the new alias and physical footprint alive.
    pub expected: Vec<u8>,
}

/// Add a literal ordinary alias and remove only the original selected name.
/// Surviving logical NUMBER is deliberately non-dense after dropping the first.
pub fn surviving_alias(source: &Source, name: &str) -> AliasCase {
    surviving_alias_with_properties(source, name, super::definition::Properties::Ordinary)
}

pub(super) fn surviving_unique_alias(source: &Source, name: &str) -> AliasCase {
    surviving_alias_with_properties(source, name, super::definition::Properties::Unique)
}

fn surviving_alias_with_properties(
    source: &Source,
    name: &str,
    properties: super::definition::Properties,
) -> AliasCase {
    assert!(name.is_ascii() && !name.is_empty() && name.len() <= 64 && name != source.index_name);
    let base = source.target_definition as usize * PAGE;
    let original = &source.expected[base..base + PAGE];
    let logical = long(original, 47);
    let physical = long(original, 51);
    let columns = 63 + physical * 12;
    let mut physical_start = columns + word(original, 45) * 25;
    for _ in 0..word(original, 45) {
        physical_start += 2 + word(original, physical_start);
    }
    let logical_start = physical_start + physical * 52;
    let names_start = logical_start + logical * 28;
    let length = long(original, 8);
    let mut at = names_start;
    let mut selected = None;
    let mut next = 0;
    for position in 0..logical {
        let end = at + 2 + word(original, at);
        let units = original[at + 2..end]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect::<Vec<_>>();
        let label = String::from_utf16(&units).unwrap();
        assert_ne!(label, name);
        if label == source.index_name {
            selected = Some(position);
        }
        next = next.max(long(original, logical_start + position * 28 + 4) + 1);
        at = end;
    }
    assert_eq!(at + 2, length);
    let selected = selected.unwrap();
    let mut alias =
        original[logical_start + selected * 28..logical_start + (selected + 1) * 28].to_vec();
    put32(&mut alias, 4, next as u32);
    let mut frame = original[..names_start].to_vec();
    frame.extend(alias);
    frame.extend_from_slice(&original[names_start..length - 2]);
    let wide = name.encode_utf16().collect::<Vec<_>>();
    frame.extend(((wide.len() * 2) as u16).to_le_bytes());
    frame.extend(wide.into_iter().flat_map(u16::to_le_bytes));
    frame.extend([255, 255]);
    let new_length = frame.len();
    assert!(new_length <= PAGE - 8);
    put32(&mut frame, 8, new_length as u32);
    put16(&mut frame, 2, (PAGE - new_length - 8) as u16);
    put32(&mut frame, 47, (logical + 1) as u32);
    let mut before = source.expected.to_vec();
    before[base..base + new_length].copy_from_slice(&frame);
    let expected = from_indexed_with_properties(source, &before, properties);
    AliasCase { before, expected }
}

/// Move the selected secondary before the retained primary in physical order.
/// Both counter/descriptor blocks and logical DATA_NUMBER references move;
/// logical NUMBER identities, names and all INDEX/map pages stay byte-identical.
pub fn reordered_physical(source: &Source) -> Source {
    let base = source.target_definition as usize * PAGE;
    let original = &source.expected[base..base + PAGE];
    assert_eq!(long(original, 47), 2);
    assert_eq!(long(original, 51), 2);
    let columns = 63 + 24;
    let mut physical = columns + word(original, 45) * 25;
    for _ in 0..word(original, 45) {
        physical += 2 + word(original, physical);
    }
    let logical = physical + 104;
    assert_eq!(long(original, physical + 52 + 38), source.root as usize);
    assert_eq!(
        long(original, physical + 52 + 34),
        (source.map_host << 8) as usize
    );
    // NO_CLONE: The reordered physical SOURCE is independent of its original;
    // no producer metadata or pages determine this literal permutation.
    let mut expected = source.expected.to_vec();
    let page = &mut expected[base..base + PAGE];
    page[63..75].copy_from_slice(&original[75..87]);
    page[75..87].copy_from_slice(&original[63..75]);
    page[physical..physical + 52].copy_from_slice(&original[physical + 52..physical + 104]);
    page[physical + 52..physical + 104].copy_from_slice(&original[physical..physical + 52]);
    for position in 0..2 {
        let reference = long(original, logical + position * 28 + 8);
        assert!(reference <= 1);
        put32(page, logical + position * 28 + 8, (1 - reference) as u32);
    }
    Source {
        before: source.before.to_vec(),
        expected,
        table: source.table,
        column: source.column,
        index_name: source.index_name,
        target_definition: source.target_definition,
        root: source.root,
        index_pages: source.index_pages.to_vec(),
        leaf_pages: source.leaf_pages,
        levels: source.levels,
        map_host: source.map_host,
        rows: source.rows.to_vec(),
        descending: source.descending,
    }
}

#[cfg(test)]
mod tests;
