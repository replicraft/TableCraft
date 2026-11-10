//! Independent logical-root DELETE images across current sparse histories.

use super::{
    current_insert,
    deleted_field::populated::{Case, readdition::Target},
};
use crate::fixtures::access::saved_field_rename::fitting::current_history;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const DATA: usize = 8 * PAGE;

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

/// Original current-history DELETE remains confined to its eleven-page source.
pub fn after(before: &[u8], removed: &[usize]) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    crate::fixtures::access::raw_record_delete::after(before, 7, removed)
}
