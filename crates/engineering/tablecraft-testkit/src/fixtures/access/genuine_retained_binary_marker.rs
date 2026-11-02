//! Six source-first images and original Marker requests; no engine dependency.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Batch,
    Null,
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
    pub fn updates(&self, reverse: bool) -> Vec<(usize, Value, Value)> {
        let mut updates = match self.recipe {
            Recipe::Batch => vec![
                (0, Value::Int32(17), Value::Int32(29)),
                (3, Value::Int32(68), Value::Int32(70)),
            ],
            Recipe::Null => vec![(3, Value::Int32(68), Value::Null)],
            Recipe::Noop => vec![(0, Value::Int32(17), Value::Int32(17))],
            Recipe::Empty => Vec::new(),
        };
        if reverse {
            updates.reverse();
        }
        updates
    }

    pub fn changed(&self) -> usize {
        match self.recipe {
            Recipe::Batch => 2,
            Recipe::Null => 1,
            Recipe::Noop | Recipe::Empty => 0,
        }
    }

    pub fn matched(&self) -> usize {
        self.updates(false).len()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = super::genuine_binary_indexed::original_rows();
        for (ordinal, expected, value) in self.updates(false) {
            assert_eq!(rows[ordinal][2], expected);
            rows[ordinal][2] = value;
        }
        rows
    }

    pub fn sql(&self) -> &'static str {
        match self.recipe {
            Recipe::Batch => "UPDATE BinaryKeys SET Marker=IIf(Id=1,29,70) WHERE Id=1 OR Id=4",
            Recipe::Null => "UPDATE BinaryKeys SET Marker=NULL WHERE Id=4",
            Recipe::Noop => "UPDATE BinaryKeys SET Marker=17 WHERE Id=1",
            Recipe::Empty => "UPDATE BinaryKeys SET Marker=29 WHERE Id=99",
        }
    }

    /// Independently framed TCNI0004. The caller owns the original digest kernel.
    pub fn wire(&self, binding: [u8; 32], reverse: bool) -> Vec<u8> {
        let updates = self.updates(reverse);
        let total = 72 + 10 + 12 + 6 + 26 * updates.len();
        let mut wire = Vec::from(*b"TCNI0004");
        wire.extend_from_slice(&(total as u32).to_le_bytes());
        wire.extend_from_slice(&[4, 0, 0, 0]);
        wire.extend_from_slice(&(self.before.len() as u64).to_le_bytes());
        wire.extend_from_slice(&binding);
        for number in [10_u32, 1, updates.len() as u32, 0] {
            wire.extend_from_slice(&number.to_le_bytes());
        }
        wire.extend_from_slice(b"BinaryKeys");
        for number in [6_u32, updates.len() as u32, 0] {
            wire.extend_from_slice(&number.to_le_bytes());
        }
        wire.extend_from_slice(b"Marker");
        for (row, expected, value) in updates {
            wire.extend_from_slice(&(row as u64).to_le_bytes());
            for value in [expected, value] {
                match value {
                    Value::Int32(value) => {
                        wire.push(2);
                        wire.extend_from_slice(&u64::from(value as u32).to_le_bytes());
                    }
                    Value::Null => wire.extend_from_slice(&[0; 9]),
                    _ => panic!("outside frozen Marker carrier law"),
                }
            }
        }
        assert_eq!(wire.len(), total);
        wire
    }

    /// Fixed, source-relative protected bytes. No serializer/producer geometry.
    pub fn forged_offsets(&self) -> Vec<usize> {
        let data = self.data as usize * 4096;
        let mut offsets = vec![
            20,
            self.definition as usize * 4096 + 16,
            data + 2,
            data + 12,
            data + 14,
            data + 4079 + 2,
            data + 4079 + 10,
            data + 4026 + 6,
            data + 4043,
        ];
        for root in self.roots {
            offsets.extend([root as usize * 4096 + 22, root as usize * 4096 + 480]);
        }
        // Held read reports bind GLOBAL FREE ref256 and the five native
        // table/index map records at (TD+1):slots0..4. These are source roles,
        // not producer-derived placements or allocation permission.
        for (page, slot) in std::iter::once((1_usize, 0_usize))
            .chain((0..5).map(|slot| (self.definition as usize + 1, slot)))
        {
            let base = page * 4096;
            let word_at = base + 14 + 2 * slot;
            let word = u16::from_le_bytes(self.before[word_at..word_at + 2].try_into().unwrap());
            assert_eq!(word & 0xe000, 0);
            let start = usize::from(word);
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
        offsets.push(self.before.len() - 4096);
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }

    pub fn assert_complement(&self) {
        assert_eq!(self.before.len(), self.after.len());
        let base = self.data as usize * 4096;
        let allowed = match self.recipe {
            Recipe::Batch => vec![base + 4085, base + 4032],
            Recipe::Null => vec![base + 4043],
            Recipe::Noop | Recipe::Empty => Vec::new(),
        };
        let actual: Vec<_> = self
            .before
            .iter()
            .zip(&self.after)
            .enumerate()
            .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
            .collect();
        let mut expected = allowed;
        expected.sort_unstable();
        assert_eq!(actual, expected);
        for root in self.roots {
            let span = root as usize * 4096..(root as usize + 1) * 4096;
            assert_eq!(self.before[span.clone()], self.after[span]);
        }
        if self.recipe == Recipe::Null {
            assert_eq!(&self.after[base + 4032..base + 4036], &68_i32.to_le_bytes());
            assert_eq!(self.before[base + 4043], 7);
            assert_eq!(self.after[base + 4043], 3);
        }
    }
}

/// Load and verify all six images before returning any case to a consumer.
pub fn load_all(mut check: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(
        std::env::var_os("TABLECRAFT_RETAINED_BINARY_MARKER_SOURCE_DIR")
            .expect("independent Marker source directory"),
    );
    assert!(dir.is_absolute());
    let mut views = Vec::new();
    for (header, length, pins) in [
        (
            5,
            393216,
            [
                "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
                "0ae675e2f1ad8c3ad4da10b7fdb7278723ee2c0e53e6226d0d0eb1deca1fa54d",
                "62bc4aac31a68beb4deba0ba85d7b83034c405f95c684657e33fb75e24ec94f2",
            ],
        ),
        (
            6,
            462848,
            [
                "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
                "c5da4b88d7b55260d68e0aaf8450e8579f0d684edb2243d37683fd6b0bc25b47",
                "14235ebea47ba225c89888a4c96e66494d477a9e8c634e5f61d85c69064dbb68",
            ],
        ),
    ] {
        for (recipe, pin) in ["before", "batch", "marker-null"].into_iter().zip(pins) {
            let raw = fs::read(dir.join(format!("h{header}-retained-{recipe}.accdb"))).unwrap();
            assert_eq!(raw.len(), length);
            check(&raw, pin);
            assert_eq!(&raw[20..24], &u32::from(header).to_le_bytes());
            views.push((header, recipe, raw));
        }
    }
    let mut cases = Vec::new();
    for header in [5, 6] {
        let view = |recipe| {
            views
                .iter()
                .find(|item| item.0 == header && item.1 == recipe)
                .unwrap()
                .2
                .clone()
        };
        for recipe in [Recipe::Batch, Recipe::Null, Recipe::Noop, Recipe::Empty] {
            let case = Case {
                header,
                recipe,
                before: view("before"),
                after: view(match recipe {
                    Recipe::Batch => "batch",
                    Recipe::Null => "marker-null",
                    Recipe::Noop | Recipe::Empty => "before",
                }),
                definition: if header == 5 { 89 } else { 106 },
                data: if header == 5 { 94 } else { 111 },
                roots: if header == 5 {
                    [91, 92, 93]
                } else {
                    [108, 109, 110]
                },
            };
            case.assert_complement();
            cases.push(case);
        }
    }
    cases
}
