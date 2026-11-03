//! Qualified unindexed GUID originals and independent literal native spans.
use super::{
    canonical_ordinary_wide_insert as original,
    original_fixed_wire::{self, Dialect, Group},
};
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Guid, Value};

pub const OLD_CANONICAL: [u8; 16] = [
    0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const OLD_NATIVE: [u8; 16] = [
    0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const NEW_CANONICAL: [u8; 16] = [
    0x6b, 0x29, 0xfc, 0x40, 0xca, 0x47, 0x10, 0x67, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
];
const NEW_NATIVE: [u8; 16] = [
    0x40, 0xfc, 0x29, 0x6b, 0x47, 0xca, 0x67, 0x10, 0xb3, 0x1d, 0, 0xdd, 1, 6, 0x62, 0xda,
];
const PINS: &[(u8, &str)] = &[
    (
        5,
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        6,
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    Present,
    NullClear,
    NullRestore,
}

pub struct Case {
    pub header: u8,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

impl Case {
    pub fn request(&self) -> (usize, Value, Value) {
        match self.recipe {
            Recipe::Present => (
                2,
                Value::Guid(Guid::from_bytes(OLD_CANONICAL)),
                Value::Guid(Guid::from_bytes(NEW_CANONICAL)),
            ),
            Recipe::NullClear => (2, Value::Guid(Guid::from_bytes(OLD_CANONICAL)), Value::Null),
            Recipe::NullRestore => (2, Value::Null, Value::Guid(Guid::from_bytes(OLD_CANONICAL))),
        }
    }

    pub fn rows(&self, after: bool) -> Vec<Vec<Value>> {
        let source = original::source(original::Case::FixedExact);
        let mut rows = source.rows_before;
        rows.push(source.values);
        if self.recipe == Recipe::NullRestore && !after {
            rows[2][9] = Value::Null;
        }
        if after {
            rows[2][9] = self.request().2;
        }
        rows
    }

    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        let updates = [self.request()];
        original_fixed_wire::original_wire(
            Dialect::Guid,
            "Items",
            &[Group {
                column: "GuidValue",
                updates: &updates,
            }],
            self.before.len(),
            binding,
        )
    }

    pub fn forgeries(&self) -> Vec<(usize, u8)> {
        let record = 8 * 4096 + 3907;
        vec![
            (0, 1),
            (4096 + 100, 1),
            (7 * 4096 + 16, 1),
            (8 * 4096 + 2, 1),
            (8 * 4096 + 14, 1),
            (8 * 4096 + 4033 + 45, 1),
            (record + 45, 1),
            (record + 49, 1),
            (record + 51, 1),
            (record + 60, 1),
            (record + 62, 2),
            (self.after.len() - 1, 1),
        ]
    }
}

/// Both original files are pinned before any manual variant is returned.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_WIDE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let originals: Vec<_> = PINS
        .iter()
        .map(|&(header, hash)| {
            let raw =
                fs::read(directory.join(format!("h{header}-fixed-exact-after.accdb"))).unwrap();
            assert_eq!(raw.len(), 9 * 4096);
            verify(&raw, hash);
            (header, raw)
        })
        .collect();
    let mut cases = Vec::new();
    for (header, before) in originals {
        let record = 8 * 4096 + 3907;
        assert_eq!(&before[record + 45..record + 61], &OLD_NATIVE);
        assert_eq!(&before[record + 61..record + 63], &[0xff, 3]);
        let mut present = before.clone();
        present[record + 45..record + 61].copy_from_slice(&NEW_NATIVE);
        let mut null = before.clone();
        null[record + 62] = 1;
        assert_eq!(&null[record + 45..record + 61], &OLD_NATIVE);
        for (recipe, original, after) in [
            (Recipe::Present, before.clone(), present),
            (Recipe::NullClear, before.clone(), null.clone()),
            (Recipe::NullRestore, null, before),
        ] {
            for (offset, (&old, &new)) in original.iter().zip(&after).enumerate() {
                let selected = if recipe == Recipe::Present {
                    (record + 45..record + 61).contains(&offset)
                } else {
                    offset == record + 62
                };
                assert!(selected || old == new);
            }
            cases.push(Case {
                header,
                recipe,
                before: original,
                after,
            });
        }
    }
    cases
}
