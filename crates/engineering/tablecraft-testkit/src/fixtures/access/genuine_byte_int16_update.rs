//! Existing exact originals and independently specified fixed payload/mask spans.
use super::{
    canonical_ordinary_wide_insert as original,
    original_fixed_wire::{self, Dialect, Group},
};
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

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
    Endpoints,
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
    pub fn requests(&self) -> Vec<(&'static str, Value, Value)> {
        match self.recipe {
            Recipe::NullRestore => vec![
                ("ByteValue", Value::Null, Value::Byte(255)),
                ("Int16Value", Value::Null, Value::Int16(-32_768)),
            ],
            recipe => {
                let (byte, integer) = if recipe == Recipe::Endpoints {
                    (Value::Byte(0), Value::Int16(32_767))
                } else {
                    (Value::Null, Value::Null)
                };
                vec![
                    ("ByteValue", Value::Byte(255), byte),
                    ("Int16Value", Value::Int16(-32_768), integer),
                ]
            }
        }
    }

    pub fn rows(&self, after: bool) -> Vec<Vec<Value>> {
        let source = original::source(original::Case::FixedExact);
        let mut rows = source.rows_before;
        rows.push(source.values);
        if self.recipe == Recipe::NullRestore {
            if !after {
                rows[2][1] = Value::Null;
                rows[2][2] = Value::Null;
            }
        } else if after {
            for (position, (_, _, value)) in self.requests().into_iter().enumerate() {
                rows[2][position + 1] = value;
            }
        }
        rows
    }

    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        let requests = self.requests();
        let updates: Vec<_> = requests
            .iter()
            .map(|(_, before, after)| vec![(2, before.clone(), after.clone())])
            .collect();
        let groups: Vec<_> = requests
            .iter()
            .zip(&updates)
            .map(|((column, _, _), updates)| Group { column, updates })
            .collect();
        original_fixed_wire::original_wire(
            Dialect::Scalar,
            "Items",
            &groups,
            self.before.len(),
            binding,
        )
    }

    pub fn forged_offsets(&self) -> Vec<usize> {
        vec![
            0,
            4096 + 100,
            7 * 4096 + 16,
            8 * 4096 + 2,
            8 * 4096 + 14,
            8 * 4096 + 4033 + 2,
            8 * 4096 + 3907 + 2,
            8 * 4096 + 3907 + 3,
            8 * 4096 + 3907 + 61,
            self.after.len() - 1,
        ]
    }

    pub fn sql(&self) -> &'static str {
        match self.recipe {
            Recipe::Endpoints => {
                "UPDATE Items SET [ByteValue]=0,[Int16Value]=32767 WHERE [Int32Value]=17"
            }
            Recipe::NullClear => {
                "UPDATE Items SET [ByteValue]=NULL,[Int16Value]=NULL WHERE [Int32Value]=17"
            }
            Recipe::NullRestore => {
                "UPDATE Items SET [ByteValue]=CByte(255),[Int16Value]=CInt(-32768) WHERE [Int32Value]=17"
            }
        }
    }
}

/// Both source files are pinned before returning any operation context.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_BYTE_INT16_UPDATE_SOURCE_DIR").unwrap());
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
        assert_eq!(&before[record..record + 2], &10_u16.to_le_bytes());
        assert_eq!(&before[record + 2..record + 5], &[0xff, 0, 0x80]);
        assert_eq!(&before[record + 61..record + 63], &[0xff, 3]);
        for recipe in [Recipe::Endpoints, Recipe::NullClear] {
            let mut after = before.clone();
            match recipe {
                Recipe::Endpoints => {
                    after[record + 2..record + 5].copy_from_slice(&[0, 0xff, 0x7f])
                }
                Recipe::NullClear => after[record + 61] = 0xf9,
                Recipe::NullRestore => unreachable!(),
            }
            assert_eq!(after.len(), before.len());
            for (offset, (&old, &new)) in before.iter().zip(&after).enumerate() {
                let selected = if recipe == Recipe::Endpoints {
                    (record + 2..record + 5).contains(&offset)
                } else {
                    offset == record + 61
                };
                assert!(selected || old == new);
            }
            if recipe == Recipe::NullClear {
                assert_eq!(&after[record + 2..record + 5], &[0xff, 0, 0x80]);
            }
            cases.push(Case {
                header,
                recipe,
                before: before.clone(),
                after,
            });
        }
        let mut absent = before.clone();
        absent[record + 61] = 0xf9;
        assert_eq!(&absent[record + 2..record + 5], &[0xff, 0, 0x80]);
        cases.push(Case {
            header,
            recipe: Recipe::NullRestore,
            before: absent,
            after: before,
        });
    }
    cases
}
