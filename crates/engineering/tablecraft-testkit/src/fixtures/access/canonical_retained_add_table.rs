//! Independent append-only tables beside retained current schemas and tombstones.
pub mod desktop;
use super::{
    canonical_creation, canonical_creation_primary, canonical_initial_table_creation as initial,
    canonical_native17_creation, packed_record::append_record, put16, put32,
};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const MAPS: usize = 3 * PAGE;
const CATALOG: usize = 4 * PAGE;
const CATALOG_LIVE: usize = 2 * PAGE + 16;

#[derive(Clone, Copy, Debug)]
pub enum Target {
    GeneratedNumber,
    GeneratedText,
    ManualNumber,
    KeylessNumber,
}

pub const TARGETS: [Target; 4] = [
    Target::GeneratedNumber,
    Target::GeneratedText,
    Target::ManualNumber,
    Target::KeylessNumber,
];

impl Target {
    pub const fn keyed(self) -> bool {
        !matches!(self, Self::KeylessNumber)
    }

    pub const fn pages(self) -> usize {
        if self.keyed() { 3 } else { 2 }
    }
}

fn template(target: Target) -> Vec<u8> {
    match target {
        Target::GeneratedNumber => initial::source(initial::Case::LongInteger).bytes,
        Target::GeneratedText => initial::source(initial::Case::ShortText).bytes,
        Target::ManualNumber => {
            canonical_creation_primary::source(canonical_creation_primary::Case::Empty).bytes
        }
        Target::KeylessNumber => canonical_native17_creation::source(
            canonical_creation::source(canonical_creation::Case::Empty).bytes,
        ),
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Append fresh pages and physical metadata slots. Deleted native identities,
/// slots, payloads and property snapshots are never recycled or reconstructed.
pub fn append(before: &[u8], name: &str, target: Target) -> Vec<u8> {
    append_profile(before, name, target, false)
}

fn append_profile(before: &[u8], name: &str, target: Target, desktop: bool) -> Vec<u8> {
    assert_eq!(before.len() % PAGE, 0);
    assert!(!name.is_empty());
    let definition = before.len() / PAGE;
    let owned = word(before, MAPS + 12);
    let old_live = u32::from_le_bytes(before[CATALOG_LIVE..CATALOG_LIVE + 4].try_into().unwrap());
    let mut bytes = before.to_vec();

    for member in [definition + 1, definition + 1]
        .into_iter()
        .chain(target.keyed().then_some(definition + 2))
    {
        let mut map = [0; 13];
        let base = member / 64 * 64;
        put32(&mut map, 1, base as u32);
        map[5 + (member - base) / 8] = 1 << ((member - base) % 8);
        append_record(&mut bytes[MAPS..MAPS + PAGE], &map);
    }
    let raw = canonical_creation::catalog_record(definition as u32, 0x0f00_0001, name, 1, 0);
    let row = canonical_native17_creation::catalog_row(&raw);
    let row = if desktop {
        super::canonical_creation_properties::attach(&row, &self::desktop::properties(target))
    } else {
        row
    };
    append_record(&mut bytes[CATALOG..CATALOG + PAGE], &row);
    put32(&mut bytes, CATALOG_LIVE, old_live + 1);

    let original = template(target);
    let mut declaration = original[DEFINITION..DEFINITION + PAGE].to_vec();
    if desktop {
        let first_name = 75 + 25 * word(&declaration, 45);
        assert_eq!(
            &declaration[first_name..first_name + 6],
            &[4, 0, b'I', 0, b'd', 0]
        );
        declaration[first_name + 4] = b'D';
    }
    put32(&mut declaration, 55, (3 << 8) | owned as u32);
    put32(&mut declaration, 59, (3 << 8) | (owned + 1) as u32);
    if target.keyed() {
        let count = word(&declaration, 45);
        let mut index = 75 + 25 * count;
        for _ in 0..count {
            index += 2 + word(&declaration, index);
        }
        put32(&mut declaration, index + 34, (3 << 8) | (owned + 2) as u32);
        put32(&mut declaration, index + 38, (definition + 2) as u32);
    }
    if desktop {
        self::desktop::definition(&mut declaration);
    }
    bytes.extend(declaration);
    let mut data = original[8 * PAGE..9 * PAGE].to_vec();
    put32(&mut data, 4, definition as u32);
    bytes.extend(data);
    if target.keyed() {
        let mut root = original[9 * PAGE..10 * PAGE].to_vec();
        put32(&mut root, 4, definition as u32);
        bytes.extend(root);
    }
    assert_eq!(bytes.len(), before.len() + target.pages() * PAGE);
    extend_global_map(&mut bytes, before.len() / PAGE);
    assert_eq!(&bytes[7 * PAGE..before.len()], &before[7 * PAGE..]);
    // Explicit empty pages keep their source-template free-space accounting.
    assert_eq!(word(&bytes, (definition + 1) * PAGE + 12), 0);
    bytes
}

fn extend_global_map(bytes: &mut [u8], old_pages: usize) {
    let pages = bytes.len() / PAGE;
    let page = &mut bytes[PAGE..2 * PAGE];
    assert_eq!(&page[..2], &[1, 1]);
    assert_eq!(word(page, 12), 1);
    let old_start = word(page, 14);
    let old = page[old_start..].to_vec();
    assert_eq!(&old[..5], &[0; 5]);
    let length = old.len().max(5 + pages.div_ceil(8));
    let start = PAGE - length;
    assert!(page[start..old_start].iter().all(|byte| *byte == 0));
    let mut map = old;
    map.resize(length, 0);
    for allocated in old_pages..pages {
        map[5 + allocated / 8] &= !(1 << (allocated % 8));
    }
    page[start..].copy_from_slice(&map);
    put16(page, 14, start as u16);
    put16(page, 2, (start - 16) as u16);
}
