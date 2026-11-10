//! Logical table DROP over independently constructed populated current histories.
use super::super::{
    canonical_blank_creation,
    canonical_created_number::mixed::after_off::{
        current_delete,
        deleted_field::populated::{self, Case, readdition::Target},
    },
};

const PAGE: usize = 4096;
const USER_DEFINITION: usize = 7 * PAGE;
const CATALOG_DEFINITION: usize = 2 * PAGE;
const CATALOG_DATA: usize = 4 * PAGE;
const LIVE: usize = 16;
const USER_DIRECTORY: usize = CATALOG_DATA + 14 + 2 * 7;

#[derive(Clone, Copy, Debug)]
pub enum History {
    Initial,
    Fields,
    Inserted,
    Deleted,
    Empty,
}

pub const HISTORIES: [History; 5] = [
    History::Initial,
    History::Fields,
    History::Inserted,
    History::Deleted,
    History::Empty,
];

pub struct Source {
    pub bytes: Vec<u8>,
    pub roots: Vec<usize>,
}

pub fn source(case: Case, target: Target, filled: bool, history: History) -> Source {
    let (bytes, roots) = if matches!(history, History::Initial) {
        let bytes = populated::source(case);
        let count = u32::from_le_bytes(
            bytes[USER_DEFINITION + LIVE..USER_DEFINITION + LIVE + 4]
                .try_into()
                .unwrap(),
        );
        (bytes, (0..count as usize).collect())
    } else {
        let original = current_delete::source(
            case,
            target,
            filled,
            true,
            matches!(
                history,
                History::Inserted | History::Deleted | History::Empty
            ),
        );
        let removed = match history {
            History::Deleted => original.roots[..1].to_vec(),
            History::Empty => original.roots.to_vec(),
            _ => Vec::new(),
        };
        let bytes = current_delete::after(&original.bytes, &removed);
        let roots = original
            .roots
            .into_iter()
            .filter(|root| !removed.contains(root))
            .collect();
        (bytes, roots)
    };
    // These exact seed declarations have no rows and their owned/candidate map
    // records are empty. Field/row histories never create permission or relation
    // entries; the independent DROP therefore has no such entries to remove.
    let seed = canonical_blank_creation::source();
    assert_eq!(&bytes[5 * PAGE..7 * PAGE], &seed[5 * PAGE..7 * PAGE]);
    assert_empty_system_maps(&bytes, &seed);
    Source { bytes, roots }
}

fn assert_empty_system_maps(bytes: &[u8], seed: &[u8]) {
    // User map cells legitimately differ from the blank seed. System table5/6
    // map cells are original records1/2 and5/6 in the shared page3 directory.
    for slot in [1, 2, 5, 6] {
        let at = 3 * PAGE + 14 + slot * 2;
        let offset = usize::from(u16::from_le_bytes(seed[at..at + 2].try_into().unwrap()));
        assert_eq!(&bytes[3 * PAGE + offset..3 * PAGE + offset + 13], &[0; 13]);
    }
}

/// Delete every actual user logical root/index membership, then the catalog
/// root. Storage, generated history, opaque payloads and allocation stay retained.
pub fn after(source: &Source) -> Vec<u8> {
    let mut bytes = current_delete::after(&source.bytes, &source.roots);
    assert_eq!(
        &bytes[USER_DEFINITION + LIVE..USER_DEFINITION + LIVE + 4],
        &[0; 4]
    );
    let count = CATALOG_DEFINITION + LIVE;
    assert_eq!(&bytes[count..count + 4], &[9, 0, 0, 0]);
    let old = u16::from_le_bytes(
        bytes[USER_DIRECTORY..USER_DIRECTORY + 2]
            .try_into()
            .unwrap(),
    );
    // Catalog metadata growth may already forward this live logical root.
    // Deletion marks that root, retaining its target and all prior payload.
    assert_eq!(old & 0x8000, 0);
    bytes[USER_DIRECTORY..USER_DIRECTORY + 2].copy_from_slice(&(old | 0xc000).to_le_bytes());
    bytes[count..count + 4].copy_from_slice(&8u32.to_le_bytes());
    bytes
}
