//! Independent logical-root deletion and ID-leaf compaction at actual table roles.
mod membership;

use super::{put16, put32};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const LIVE: usize = 16;
const FREE: usize = 2;
const BITMAP: usize = 27;
const ENTRIES: usize = 480;
const KEY_BYTES: usize = 9;
const TOMBSTONE: u16 = 0xc000;

/// Only the selected logical directory words, live count and active ID leaf
/// change. Terminal records, hidden history and newly inactive key bytes survive.
pub(in crate::fixtures::access) fn after(
    before: &[u8],
    definition_page: u32,
    removed: &[usize],
) -> Vec<u8> {
    let definition = definition_page as usize * PAGE;
    let data = (definition_page as usize + 1) * PAGE;
    let index = (definition_page as usize + 2) * PAGE;
    let original = membership::capture(before, definition_page);
    for (position, root) in removed.iter().enumerate() {
        assert!(!removed[..position].contains(root));
        assert_eq!(
            original.iter().filter(|entry| entry.root == *root).count(),
            1
        );
    }
    let retained: Vec<_> = original
        .iter()
        .filter(|entry| !removed.contains(&entry.root))
        .collect();
    if removed.is_empty() {
        return before.to_vec();
    }
    let mut output = before.to_vec();
    for root in removed {
        let at = data + DIRECTORY + 2 * root;
        let old = u16::from_le_bytes(before[at..at + 2].try_into().unwrap());
        put16(&mut output, at, old | TOMBSTONE);
    }
    put32(&mut output, definition + LIVE, retained.len() as u32);
    put16(
        &mut output,
        index + FREE,
        (PAGE - ENTRIES - retained.len() * KEY_BYTES) as u16,
    );
    output[index + BITMAP..index + ENTRIES].fill(0);
    for (position, entry) in retained.iter().enumerate() {
        let at = index + ENTRIES + position * KEY_BYTES;
        output[at..at + KEY_BYTES].copy_from_slice(entry.bytes);
        let end = (position + 1) * KEY_BYTES;
        output[index + BITMAP + end / 8] |= 1 << (end % 8);
    }
    output
}
