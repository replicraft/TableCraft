//! Pinned independently read-qualified sources and literal TCND original witnesses.
use super::budget;
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};
use tablecraft_ace::{NativeDeleteIntent, RowDelete};
use tablecraft_core::values::Value;

const AFTER_PINS: [(u8, &str, &str); 8] = [
    (
        5,
        "null-first",
        "5ac960784298110151a18068598ed010179241be76e96aea12a69d5fdc5a688e",
    ),
    (
        5,
        "present-middle",
        "eec3009356de6fca540fae4c6b48188fcaa4a3a7581d13f03092a671efe83f20",
    ),
    (
        5,
        "exterior",
        "16d90236e802bb484187876f90aa7e1ca1fd97807818a4b970efb3348b890a0b",
    ),
    (
        5,
        "all",
        "87a71e52cd0d5d0bb5ae1d13a4140b9cc715a57a0d6e83f4fadaa6da85669a2d",
    ),
    (
        6,
        "null-first",
        "60aa0a895fc4e7f13fb1c6c492eee5796bc0b67705b2797f38b2c1ad7ea8671a",
    ),
    (
        6,
        "present-middle",
        "d7e88b430f11694d1675e93b2e26c54e5baa1975013cb3a9c40b9735e14a8eaf",
    ),
    (
        6,
        "exterior",
        "8d4e8a882078fea639f09cba9e340ae36ffa90f23c88946fedf17da4d4a80ab5",
    ),
    (
        6,
        "all",
        "0e226ffebf9de008b61620175cc0a65654fc19ff6325632abd87f10bcc02ceea",
    ),
];

pub(super) struct Case {
    pub(super) recipe: &'static str,
    pub(super) selected: &'static [usize],
    pub(super) before: Vec<u8>,
    pub(super) after: Vec<u8>,
    pub(super) definition: usize,
    pub(super) data: usize,
    pub(super) roots: [usize; 2],
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn record(slot: usize) -> Vec<u8> {
    let (values, mask): ([i32; 3], u8) = match slot {
        0 => ([1, -1, 0], 3), // NULL Guard has literal zero inactive payload, not Int32(0).
        1 => ([2, 0, 1], 7),
        2 => ([3, 1, 2], 7),
        _ => panic!("outside frozen source rows"),
    };
    let mut raw = vec![3, 0];
    for value in values {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.push(mask);
    raw
}

pub(super) fn cases() -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_DESC_NULL_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read(directory.join("manifest.json")).unwrap();
    assert_eq!(
        hash(&manifest),
        "5cb3b6ec5efe19b7d1f73cfa6090ae5b57cc6cc489880cd0edda574ca8eb353c"
    );
    // Load and pin every source outcome before any planner or journal capability.
    AFTER_PINS
        .into_iter()
        .map(|(header, recipe, after_hash)| {
            let name = format!("h{header}-desc-null-delete-{recipe}");
            let before = fs::read(directory.join(format!("{name}-before.accdb"))).unwrap();
            let after = fs::read(directory.join(format!("{name}-deleted.accdb"))).unwrap();
            let (bytes, definition, data, roots, before_hash) = match header {
                5 => (
                    389120,
                    89,
                    93,
                    [91, 92],
                    "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
                ),
                6 => (
                    458752,
                    106,
                    110,
                    [108, 109],
                    "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
                ),
                _ => unreachable!(),
            };
            assert_eq!(before.len(), bytes);
            assert_eq!(after.len(), bytes);
            assert_eq!(hash(&before), before_hash);
            assert_eq!(hash(&after), after_hash);
            for slot in 0..3 {
                let start = data * 4096 + 4081 - 15 * slot;
                assert_eq!(&before[start..start + 15], record(slot));
                assert_eq!(&after[start..start + 15], record(slot));
            }
            let selected: &[usize] = match recipe {
                "null-first" => &[0],
                "present-middle" => &[1],
                "exterior" => &[0, 2],
                "all" => &[0, 1, 2],
                _ => unreachable!(),
            };
            Case {
                recipe,
                selected,
                before,
                after,
                definition,
                data,
                roots,
            }
        })
        .collect()
}

pub(super) fn ordinals(case: &Case, reverse: bool) -> Vec<usize> {
    let mut selected = case.selected.to_vec();
    if reverse {
        assert_eq!(case.recipe, "exterior");
        selected.reverse();
    }
    selected
}

pub(super) fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    ordinals(case, reverse)
        .into_iter()
        .map(|slot| RowDelete::new(slot, record(slot)))
        .collect()
}

pub(super) fn rows(case: &Case, descending_guard: bool) -> Vec<Vec<Value>> {
    let order = if descending_guard {
        [2, 1, 0]
    } else {
        [0, 1, 2]
    };
    order
        .into_iter()
        .filter(|slot| !case.selected.contains(slot))
        .map(|slot| match slot {
            0 => vec![Value::Int32(1), Value::Int32(-1), Value::Null],
            1 => vec![Value::Int32(2), Value::Int32(0), Value::Int32(1)],
            2 => vec![Value::Int32(3), Value::Int32(1), Value::Int32(2)],
            _ => unreachable!(),
        })
        .collect()
}

pub(super) fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let selected = ordinals(case, reverse);
    let total: u32 = match selected.len() {
        1 => 100,
        2 => 127,
        3 => 154,
        _ => unreachable!(),
    };
    let mut raw = Vec::from(*b"TCND0001");
    raw.extend_from_slice(&total.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for number in [5_u32, selected.len() as u32, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for slot in selected {
        raw.extend_from_slice(&(slot as u64).to_le_bytes());
        raw.extend_from_slice(&15_u32.to_le_bytes());
        raw.extend_from_slice(&record(slot));
    }
    assert_eq!(raw.len(), total as usize);
    raw
}

pub(super) fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, false), &mut budget(), &|| false).unwrap()
}
