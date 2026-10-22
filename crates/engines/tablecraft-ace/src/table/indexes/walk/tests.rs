//! Ownership regressions use native numeric pages and a deliberately non-Clone key.
//! These tests qualify traversal mechanics, not text weights or Access fixtures.
use super::*;
use crate::table::indexes::key::KeyWork;
use crate::table::indexes::key::TestKeyProfile;
use crate::{
    Header,
    table::indexes::{
        Int64IndexSelection,
        key::{KeyRow, SingleKey},
        profile::KeyType,
    },
};
use std::{io::Cursor, ops::Bound::Included};
use tablecraft_core::{limits::ResourceBudget, values::Value};

#[derive(Eq, PartialEq)]
struct OwnedKey(Option<i64>);

struct OwnedProfile(SingleKey);
impl KeyProfile for OwnedProfile {
    type Key = OwnedKey;
    fn components(&self) -> usize {
        self.0.components()
    }
    fn max_key_bytes(&self) -> usize {
        self.0.max_key_bytes()
    }
    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(OwnedKey, usize), InspectError> {
        self.0
            .decode(bytes, work)
            .map(|(key, end)| (OwnedKey(key), end))
    }
    fn compare(&self, left: &OwnedKey, right: &OwnedKey) -> Ordering {
        self.0.compare(&left.0, &right.0)
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<OwnedKey, InspectError> {
        self.0.row_key(row, work).map(OwnedKey)
    }
    fn constraint_nulls(&self, key: &OwnedKey) -> Result<(bool, bool), InspectError> {
        Ok((key.0.is_none(), key.0.is_none()))
    }
    fn contains(&self, key: &OwnedKey) -> bool {
        self.0.contains(&key.0)
    }
    fn complete(&self) -> bool {
        self.0.complete()
    }
    fn may_match(
        &self,
        lower: Option<&KeyRef<OwnedKey>>,
        upper: Option<&KeyRef<OwnedKey>>,
    ) -> bool {
        // Only the compact scalar test representation is copied; OwnedKey itself
        // cannot be copied or cloned by the walker.
        let lower = lower.map(|entry| KeyRef {
            key: entry.key.0,
            row: entry.row,
        });
        let upper = upper.map(|entry| KeyRef {
            key: entry.key.0,
            row: entry.row,
        });
        self.0.may_match(lower.as_ref(), upper.as_ref())
    }
}

fn tree() -> Vec<u8> {
    let mut bytes = vec![0; 5 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    // Root separator equals the left leaf's final key + row reference.
    for (number, kind, previous, next, tail, entry) in [
        (
            2,
            3,
            0_u32,
            0_u32,
            4_u32,
            &[0x7f, 0x80, 0, 0, 1, 0, 0, 3, 0, 0, 0, 0, 3][..],
        ),
        (3, 4, 0, 4, 0, &[0x7f, 0x80, 0, 0, 1, 0, 0, 3, 0][..]),
        (4, 4, 3, 0, 0, &[0x7f, 0x80, 0, 0, 2, 0, 0, 4, 0][..]),
    ] {
        let page = &mut bytes[number * 4096..(number + 1) * 4096];
        page[..2].copy_from_slice(&[kind, 1]);
        page[2..4].copy_from_slice(&((4096 - 480 - entry.len()) as u16).to_le_bytes());
        page[4..8].copy_from_slice(&1_u32.to_le_bytes());
        page[12..16].copy_from_slice(&previous.to_le_bytes());
        page[16..20].copy_from_slice(&next.to_le_bytes());
        page[20..24].copy_from_slice(&tail.to_le_bytes());
        page[480..480 + entry.len()].copy_from_slice(entry);
        page[27 + entry.len() / 8] |= 1 << (entry.len() % 8);
    }
    bytes
}

fn traverse(
    bytes: Vec<u8>,
    selection: Int64IndexSelection,
) -> Result<(Vec<KeyRef<OwnedKey>>, Vec<u32>), InspectError> {
    let header = Header::parse(&bytes[..66], bytes.len() as u64)?;
    let mut source = Cursor::new(bytes);
    let mut budget = ResourceBudget::new(1_000_000, 100, 100_000);
    let mut reader = Reader {
        source: &mut source,
        header,
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    let profile = OwnedProfile(SingleKey {
        column: 0,
        key_type: KeyType::Int32,
        descending: false,
        selection,
    });
    let mut walker = Walker::<_, _, { page::MAX_ENTRY_BYTES }> {
        reader: &mut reader,
        owner: 1,
        owned: &[2, 3, 4],
        profile: &profile,
        visited: Vec::new(),
        entries: Vec::new(),
        maximum_rows: crate::limits::MAX_LIVE_ROWS,
        previous_leaf: 0,
        next_leaf: 0,
        gap: false,
    };
    walker.visit(2, 0, None, None)?;
    Ok((walker.entries, walker.visited))
}

#[test]
fn non_clone_keys_cross_parent_bounds_and_move_into_results() {
    let (entries, visited) = traverse(tree(), Int64IndexSelection::All).unwrap();
    assert_eq!(visited, [2, 3, 4]);
    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.key.0, entry.row))
            .collect::<Vec<_>>(),
        [(Some(1), 768), (Some(2), 1024)]
    );
    let profile = OwnedProfile(SingleKey {
        column: 0,
        key_type: KeyType::Int32,
        descending: false,
        selection: Int64IndexSelection::All,
    });
    assert!(profile.row_key_test(&[Value::Int32(1)]).unwrap() == entries[0].key);
}

#[test]
fn borrowed_parent_bounds_still_prune_unselected_subtrees() {
    let mut bytes = tree();
    // The left child is deliberately corrupt. Selection of key 2 must prune it;
    // complete traversal must detect the corruption rather than fall back.
    bytes[3 * 4096] = 0xff;
    let selection = Int64IndexSelection::Range {
        lower: Included(2),
        upper: Included(2),
    };
    let (entries, visited) = traverse(bytes.clone(), selection).unwrap();
    assert_eq!(visited, [2, 4]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key.0, Some(2));
    assert!(matches!(
        traverse(bytes, Int64IndexSelection::All),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn owned_keys_preserve_parent_and_sibling_corruption_checks() {
    let mut bounds = tree();
    bounds[4 * 4096 + 484] = 0; // Right key now falls below the root separator.
    assert!(matches!(
        traverse(bounds, Int64IndexSelection::All),
        Err(InspectError::Malformed)
    ));
    let mut siblings = tree();
    siblings[4 * 4096 + 12..4 * 4096 + 16].fill(0);
    assert!(matches!(
        traverse(siblings, Int64IndexSelection::All),
        Err(InspectError::Malformed)
    ));
}
