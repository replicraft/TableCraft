//! Source-pinned Id replacement; independent images and physical ROW laws.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Changed,
    Noop,
    Empty,
}

pub struct Case {
    pub header: u8,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
}

impl Case {
    pub fn updates(&self) -> Vec<(usize, Value, Value)> {
        match self.recipe {
            Recipe::Changed => vec![(0, Value::Int32(1), Value::Int32(14))],
            Recipe::Noop => vec![(0, Value::Int32(1), Value::Int32(1))],
            Recipe::Empty => Vec::new(),
        }
    }

    pub fn changed(&self) -> usize {
        usize::from(self.recipe == Recipe::Changed)
    }

    pub fn matched(&self) -> usize {
        self.updates().len()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = super::genuine_binary_indexed::original_rows();
        if self.recipe == Recipe::Changed {
            rows[0][0] = Value::Int32(14);
        }
        rows
    }

    pub fn index_rows(&self, alias: &str) -> Vec<Vec<Value>> {
        let primary = if self.changed() == 1 {
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0]
        } else {
            (0..13).collect()
        };
        let order = match alias {
            "PrimaryID" | "PrimaryAlias" => primary,
            "Ascending" | "AscendingAlias" => vec![0, 1, 2, 12, 4, 10, 5, 6, 7, 8, 9, 3, 11],
            "Descending" | "DescendingAlias" => vec![1, 11, 3, 9, 8, 7, 6, 5, 10, 4, 2, 12, 0],
            _ => panic!("outside frozen six-alias inventory"),
        };
        let rows = self.rows();
        order
            .into_iter()
            .map(|position| rows[position].clone())
            .collect()
    }

    pub fn sql(&self) -> &'static str {
        match self.recipe {
            Recipe::Changed => "UPDATE BinaryKeys SET Id=14 WHERE Id=1",
            Recipe::Noop => "UPDATE BinaryKeys SET Id=1 WHERE Id=1",
            Recipe::Empty => "UPDATE BinaryKeys SET Id=14 WHERE Id=99",
        }
    }

    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        super::original_fixed_wire::single_column_original_wire(
            "BinaryKeys",
            "Id",
            &self.updates(),
            self.before.len(),
            binding,
        )
    }

    pub fn assert_preserved(&self) {
        assert_eq!(self.before.len(), self.after.len());
        let data = self.data as usize * 4096;
        let primary = self.roots[0] as usize * 4096;
        let mut expected = self.before.clone();
        if self.recipe == Recipe::Changed {
            expected[data + 4081..data + 4085].copy_from_slice(&14_i32.to_le_bytes());
            for (position, slot) in (1..13_u32).chain(std::iter::once(0)).enumerate() {
                let identity = if slot == 0 { 14 } else { slot + 1 };
                let at = primary + 480 + 9 * position;
                expected[at] = 0x7f;
                expected[at + 1..at + 5].copy_from_slice(&(identity ^ 0x8000_0000).to_be_bytes());
                expected[at + 5..at + 9].copy_from_slice(&((self.data << 8) | slot).to_be_bytes());
            }
        }
        assert_eq!(expected, self.after);
        assert_eq!(&self.after[data + 4085..data + 4089], &17_i32.to_le_bytes());
        assert_eq!(self.after[data + 4095], 5);
        for root in &self.roots[1..] {
            let span = *root as usize * 4096..(*root as usize + 1) * 4096;
            assert_eq!(self.before[span.clone()], self.after[span]);
        }
    }

    pub fn forged_offsets(&self) -> Vec<usize> {
        let data = self.data as usize * 4096;
        let td = self.definition as usize * 4096;
        let primary = self.roots[0] as usize * 4096;
        let mut offsets = vec![
            20,
            td + 16,
            td + 20,
            td + 67,
            td + 79,
            td + 91,
            data + 2,
            data + 12,
            data + 14,
            data + 4085,
            data + 4095,
            primary + 2,
            primary + 27,
            primary + 480,
            primary + 597,
        ];
        for root in &self.roots[1..] {
            offsets.extend([*root as usize * 4096 + 22, *root as usize * 4096 + 480]);
        }
        for (page, slot) in std::iter::once((1_usize, 0_usize))
            .chain((0..5).map(|slot| (self.definition as usize + 1, slot)))
        {
            let base = page * 4096;
            let word_at = base + 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                self.before[word_at..word_at + 2].try_into().unwrap(),
            ));
            let end = if slot == 0 {
                4096
            } else {
                usize::from(u16::from_le_bytes(
                    self.before[word_at - 2..word_at].try_into().unwrap(),
                ))
            };
            assert_eq!(end - start, 69);
            offsets.push(base + start + 5);
        }
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets.len(), 25);
        offsets
    }
}

/// Verify all four independent image pins before returning any target case.
pub fn load_all(mut check: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_ID_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut images = Vec::new();
    for (header, length, pins) in [
        (
            5,
            393216,
            [
                "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
                "4c25b772e1fb98e6d5de7977117d03f7dca7f4b2d4e7030669925828400fcb5e",
            ],
        ),
        (
            6,
            462848,
            [
                "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
                "740326003559f5b21913ed802867eca5d25c38dbe0247fa602767ed6bddf2f14",
            ],
        ),
    ] {
        for (recipe, hash) in ["before", "id-14"].into_iter().zip(pins) {
            let raw =
                fs::read(directory.join(format!("h{header}-binary-id-{recipe}.accdb"))).unwrap();
            assert_eq!(raw.len(), length);
            check(&raw, hash);
            images.push((header, recipe, raw));
        }
    }
    let mut cases = Vec::new();
    for header in [5, 6] {
        let view = |recipe| {
            images
                .iter()
                .find(|item| item.0 == header && item.1 == recipe)
                .unwrap()
                .2
                .clone()
        };
        for recipe in [Recipe::Changed, Recipe::Noop, Recipe::Empty] {
            let case = Case {
                header,
                recipe,
                before: view("before"),
                after: view(if recipe == Recipe::Changed {
                    "id-14"
                } else {
                    "before"
                }),
                definition: if header == 5 { 89 } else { 106 },
                data: if header == 5 { 94 } else { 111 },
                roots: if header == 5 {
                    [91, 92, 93]
                } else {
                    [108, 109, 110]
                },
            };
            case.assert_preserved();
            cases.push(case);
        }
    }
    cases
}
