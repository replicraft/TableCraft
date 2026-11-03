//! Qualified original-only Score outcomes; no engine or generation dependency.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const VIEWS: &[(u32, &str, &str, &str, &str)] = &[
    (
        5,
        "Numbered",
        "before",
        "h5-numbered-before.accdb",
        "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
    ),
    (
        5,
        "Numbered",
        "batch",
        "h5-numbered-batch.accdb",
        "92126d4eb2ad907587842e426c5a96893001e2c59f0cf24d7ec570a28941d6f4",
    ),
    (
        5,
        "Numbered",
        "score-null",
        "h5-numbered-score-null.accdb",
        "c11d3e88db0a5b62d46eb6176246bffd50939083861704ccbd361340df7050f9",
    ),
    (
        6,
        "Numbered",
        "before",
        "h6-numbered-before.accdb",
        "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
    ),
    (
        6,
        "Numbered",
        "batch",
        "h6-numbered-batch.accdb",
        "b1d3aa21d750cf4cfe45c990f70868b31ca7b9ebb253ebf4616fde6f4db5b291",
    ),
    (
        6,
        "Numbered",
        "score-null",
        "h6-numbered-score-null.accdb",
        "ebf3a25e37386dde110695c95abb53a9281b80df5408c7a861d544fbbafd80ef",
    ),
    (
        5,
        "Items",
        "before",
        "h5-random-before.accdb",
        "d45a6157d94c892b8963449796b30d5e54512de1894fa9def47c042b240c2638",
    ),
    (
        5,
        "Items",
        "change",
        "h5-random-change.accdb",
        "eaac19ac6b87a687a841d01f755ee4f49d08cc6cb0188ada2b30fa837663756d",
    ),
    (
        5,
        "Items",
        "score-null",
        "h5-random-score-null.accdb",
        "c65c24b52a0cb14fdc93e3f655a285cd809f95397803ab920899eb5e183906d1",
    ),
    (
        6,
        "Items",
        "before",
        "h6-random-before.accdb",
        "8d729549fed474fdd3a7bf3731254ec46f100275e480993614ecf78e531dad2e",
    ),
    (
        6,
        "Items",
        "change",
        "h6-random-change.accdb",
        "c91f497594e8e17d593901169b57c1c842f07a2c7b9069af618bd41b166de1be",
    ),
    (
        6,
        "Items",
        "score-null",
        "h6-random-score-null.accdb",
        "d5d3e206277cd927e50fdbb79311756cd9aa7119c8aa657b58fcbd172c0e55bf",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Changed,
    Null,
    Noop,
    Empty,
}

pub struct Case {
    pub header: u32,
    pub table: &'static str,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub data: usize,
    pub root: usize,
}

impl Case {
    pub fn numbered(&self) -> bool {
        self.table == "Numbered"
    }

    pub fn generated_position(&self) -> usize {
        usize::from(self.numbered())
    }

    pub fn score_position(&self) -> usize {
        if self.numbered() { 2 } else { 1 }
    }

    pub fn original_rows(&self) -> Vec<Vec<Value>> {
        if self.numbered() {
            vec![
                vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
            ]
        } else {
            vec![vec![Value::Int32(1), Value::Int32(10)]]
        }
    }

    pub fn orders(&self) -> &'static [bool] {
        if self.numbered() && self.recipe == Recipe::Changed {
            &[false, true]
        } else {
            &[false]
        }
    }

    pub fn updates(&self, reverse: bool) -> Vec<(usize, Value, Value)> {
        let mut updates = match self.recipe {
            Recipe::Changed if self.numbered() => vec![
                (0, Value::Int32(10), Value::Int32(17)),
                (1, Value::Int32(20), Value::Int32(23)),
            ],
            Recipe::Changed => vec![(0, Value::Int32(10), Value::Int32(17))],
            Recipe::Null if self.numbered() => vec![(1, Value::Int32(20), Value::Null)],
            Recipe::Null => vec![(0, Value::Int32(10), Value::Null)],
            Recipe::Noop => vec![(0, Value::Int32(10), Value::Int32(10))],
            Recipe::Empty => Vec::new(),
        };
        if reverse {
            updates.reverse();
        }
        updates
    }

    pub fn changed(&self) -> usize {
        match self.recipe {
            Recipe::Changed if self.numbered() => 2,
            Recipe::Changed | Recipe::Null => 1,
            Recipe::Noop | Recipe::Empty => 0,
        }
    }

    pub fn matched(&self) -> usize {
        self.updates(false).len()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = self.original_rows();
        let score = self.score_position();
        for (ordinal, expected, value) in self.updates(false) {
            assert_eq!(rows[ordinal][score], expected);
            rows[ordinal][score] = value;
        }
        rows
    }

    pub fn sql(&self) -> &'static str {
        match (self.numbered(), self.recipe) {
            (true, Recipe::Changed) => {
                "UPDATE [Numbered] SET [Score]=IIf([Id]=1,17,23) WHERE [Id]=1 OR [Id]=2"
            }
            (true, Recipe::Null) => "UPDATE [Numbered] SET [Score]=NULL WHERE [Id]=2",
            (true, Recipe::Noop) => "UPDATE [Numbered] SET [Score]=10 WHERE [Id]=1",
            (true, Recipe::Empty) => "UPDATE [Numbered] SET [Score]=17 WHERE [Id]=999",
            (false, Recipe::Changed) => "UPDATE [Items] SET [Score]=17 WHERE [Id]=1",
            (false, Recipe::Null) => "UPDATE [Items] SET [Score]=NULL WHERE [Id]=1",
            (false, Recipe::Noop) => "UPDATE [Items] SET [Score]=10 WHERE [Id]=1",
            (false, Recipe::Empty) => "UPDATE [Items] SET [Score]=17 WHERE [Id]=999",
        }
    }

    pub fn wire(&self, binding: [u8; 32], reverse: bool) -> Vec<u8> {
        let updates = self.updates(reverse);
        super::original_fixed_wire::single_column_original_wire(
            self.table,
            "Score",
            &updates,
            self.before.len(),
            binding,
        )
    }

    pub fn assert_preserved(&self) {
        let base = self.data * 4096;
        let mut expected = match self.recipe {
            Recipe::Changed if self.numbered() => vec![base + 4076, base + 4091],
            Recipe::Changed => vec![base + 4091],
            Recipe::Null if self.numbered() => vec![base + 4080],
            Recipe::Null => vec![base + 4095],
            Recipe::Noop | Recipe::Empty => Vec::new(),
        };
        expected.sort_unstable();
        let actual: Vec<_> = self
            .before
            .iter()
            .zip(&self.after)
            .enumerate()
            .filter_map(|(offset, (old, new))| (old != new).then_some(offset))
            .collect();
        assert_eq!(actual, expected);
        let td = self.definition * 4096..(self.definition + 1) * 4096;
        let index = self.root * 4096..(self.root + 1) * 4096;
        assert_eq!(self.before[td.clone()], self.after[td]);
        assert_eq!(self.before[index.clone()], self.after[index]);
        assert_eq!(
            &self.after[self.definition * 4096 + 20..self.definition * 4096 + 24],
            &(if self.numbered() { 3_i32 } else { 137 }).to_le_bytes()
        );
        if self.numbered() {
            assert_eq!(&self.after[base + 18..base + 20], &0xcfd3_u16.to_le_bytes());
            assert_eq!(
                &self.before[base + 4051..base + 4066],
                &self.after[base + 4051..base + 4066]
            );
        }
        if self.recipe == Recipe::Null {
            let at = base + if self.numbered() { 4076 } else { 4091 };
            assert_eq!(&self.before[at..at + 4], &self.after[at..at + 4]);
            assert_eq!(self.after[at + 4], if self.numbered() { 3 } else { 1 });
        }
    }

    pub fn forged_offsets(&self) -> Vec<usize> {
        let td = self.definition * 4096;
        let data = self.data * 4096;
        let root = self.root * 4096;
        let mut offsets = vec![
            20,
            td + 16,
            td + 20,
            td + 67,
            td + if self.numbered() { 115 } else { 90 },
            data + 2,
            data + 12,
            data + 14,
            data + 4087,
            data + if self.numbered() { 4080 } else { 4095 },
            root + 22,
            root + 480,
            root + 4095,
        ];
        // Literal native map-reference fields, not an engine parser or writer.
        for at in [55, 59] {
            let reference =
                u32::from_le_bytes(self.before[td + at..td + at + 4].try_into().unwrap());
            let base = (reference >> 8) as usize * 4096;
            let slot = (reference & 255) as usize;
            let word_at = base + 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                self.before[word_at..word_at + 2].try_into().unwrap(),
            ));
            assert_eq!(start & 0xe000, 0);
            let end = if slot == 0 {
                4096
            } else {
                usize::from(u16::from_le_bytes(
                    self.before[word_at - 2..word_at].try_into().unwrap(),
                ))
            };
            assert_eq!(end - start, if self.numbered() { 69 } else { 13 });
            offsets.push(base + start + 5);
        }
        if self.numbered() {
            offsets.extend([data + 18, data + 4051]);
        } else {
            offsets.push(unique(&self.before, b"MR2\0") + 4);
            let expression: Vec<_> = "GenUniqueID()"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            offsets.push(unique(&self.before, &expression));
        }
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets.len(), 17);
        offsets
    }
}

fn unique(raw: &[u8], needle: &[u8]) -> usize {
    let mut matches = raw
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, value)| (value == needle).then_some(offset));
    let at = matches.next().expect("exact source property bytes");
    assert!(matches.next().is_none());
    at
}

/// Check all twelve qualified source pins before returning any case.
pub fn load_all(mut check: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(
        std::env::var_os("TABLECRAFT_GENERATED_COMPANION_UPDATE_SOURCE_DIR").unwrap(),
    );
    assert!(dir.is_absolute());
    let mut views = Vec::new();
    for &(header, table, recipe, filename, hash) in VIEWS {
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(
            raw.len(),
            if table == "Numbered" {
                if header == 5 { 446464 } else { 520192 }
            } else {
                40960
            }
        );
        check(&raw, hash);
        assert_eq!(&raw[20..24], &header.to_le_bytes());
        views.push((header, table, recipe, raw));
    }
    let mut cases = Vec::new();
    for header in [5, 6] {
        for table in ["Numbered", "Items"] {
            let view = |recipe| {
                views
                    .iter()
                    .find(|item| item.0 == header && item.1 == table && item.2 == recipe)
                    .unwrap()
                    .3
                    .clone()
            };
            for recipe in [Recipe::Changed, Recipe::Null, Recipe::Noop, Recipe::Empty] {
                let after = view(match recipe {
                    Recipe::Changed if table == "Numbered" => "batch",
                    Recipe::Changed => "change",
                    Recipe::Null => "score-null",
                    Recipe::Noop | Recipe::Empty => "before",
                });
                let case = Case {
                    header,
                    table,
                    recipe,
                    before: view("before"),
                    after,
                    definition: if table == "Numbered" {
                        if header == 5 { 94 } else { 111 }
                    } else {
                        7
                    },
                    data: if table == "Numbered" {
                        if header == 5 { 97 } else { 115 }
                    } else {
                        8
                    },
                    root: if table == "Numbered" {
                        if header == 5 { 96 } else { 113 }
                    } else {
                        9
                    },
                };
                case.assert_preserved();
                cases.push(case);
            }
        }
    }
    assert_eq!(cases.len(), 16);
    cases
}
