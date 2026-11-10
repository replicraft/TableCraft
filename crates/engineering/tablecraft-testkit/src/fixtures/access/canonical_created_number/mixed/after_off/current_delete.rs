//! Independent logical-root DELETE images across current sparse histories.
mod membership;

use super::{
    current_insert,
    deleted_field::populated::{Case, readdition::Target},
};
use crate::fixtures::access::{put16, put32, saved_field_rename::fitting::current_history};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const DATA: usize = 8 * PAGE;
const INDEX: usize = 9 * PAGE;
const DIRECTORY: usize = 14;
const LIVE: usize = 16;
const FREE: usize = 2;
const BITMAP: usize = 27;
const ENTRIES: usize = 480;
const KEY_BYTES: usize = 9;
const TOMBSTONE: u16 = 0xc000;

pub struct Source {
    pub bytes: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub roots: Vec<usize>,
    pub next_id: i32,
}

impl Source {
    pub fn survivors<'a>(&'a self, removed: &'a [usize]) -> impl Iterator<Item = &'a [Value]> {
        self.roots
            .iter()
            .zip(&self.rows)
            .filter(move |(root, _)| !removed.contains(root))
            .map(|(_, values)| values.as_slice())
    }
}

pub fn source(case: Case, target: Target, filled: bool, renamed: bool, inserted: bool) -> Source {
    let mut bytes = current_insert::source(case, target, filled, renamed);
    let mut rows = current_history::expected(case, target, filled);
    let mut roots = (0..rows.len()).collect::<Vec<_>>();
    let mut next_id = if matches!(case, Case::NumberPresent) {
        5
    } else {
        4
    };
    if inserted {
        let physical = word(&bytes, DATA + 12);
        bytes = current_insert::append(&bytes, case, target, current_insert::Input::Omitted);
        rows.push(current_insert::row(
            case,
            target,
            current_insert::Input::Omitted,
            next_id,
        ));
        roots.push(physical);
        next_id += 1;
    }
    Source {
        bytes,
        rows,
        roots,
        next_id,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

/// Only the selected logical directory words, live count and active ID leaf
/// change. Terminal records, hidden history and newly inactive key bytes survive.
pub fn after(before: &[u8], removed: &[usize]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let original = membership::capture(before);
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
        let at = DATA + DIRECTORY + 2 * root;
        let old = u16::from_le_bytes(before[at..at + 2].try_into().unwrap());
        put16(&mut output, at, old | TOMBSTONE);
    }
    put32(&mut output, DEFINITION + LIVE, retained.len() as u32);
    put16(
        &mut output,
        INDEX + FREE,
        (PAGE - ENTRIES - retained.len() * KEY_BYTES) as u16,
    );
    output[INDEX + BITMAP..INDEX + ENTRIES].fill(0);
    for (position, entry) in retained.iter().enumerate() {
        let at = INDEX + ENTRIES + position * KEY_BYTES;
        output[at..at + KEY_BYTES].copy_from_slice(entry.bytes);
        let end = (position + 1) * KEY_BYTES;
        output[INDEX + BITMAP + end / 8] |= 1 << (end % 8);
    }
    output
}
