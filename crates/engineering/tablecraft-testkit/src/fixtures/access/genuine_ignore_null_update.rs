//! Held genuine originals and literal single-I32 omitted-key UPDATE outcomes.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    NullNew,
    NullExisting,
    PresentNull,
    RepresentedNoop,
    OmittedNoop,
    Empty,
}

#[derive(Clone, Copy, Debug)]
pub enum Refusal {
    Required,
    Unique,
    RemovedForward,
    RemovedReverse,
    CointroducedForward,
    CointroducedReverse,
}

pub const REFUSALS: [Refusal; 6] = [
    Refusal::Required,
    Refusal::Unique,
    Refusal::RemovedForward,
    Refusal::RemovedReverse,
    Refusal::CointroducedForward,
    Refusal::CointroducedReverse,
];

pub struct Case {
    pub header: u8,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 2],
}

impl Case {
    pub fn updates(&self) -> Vec<(usize, Value, Value)> {
        match self.recipe {
            Recipe::NullNew => vec![(2, Value::Null, Value::Int32(30))],
            Recipe::NullExisting => vec![(2, Value::Null, Value::Int32(10))],
            Recipe::PresentNull => vec![(0, Value::Int32(10), Value::Null)],
            Recipe::RepresentedNoop => vec![(0, Value::Int32(10), Value::Int32(10))],
            Recipe::OmittedNoop => vec![(2, Value::Null, Value::Null)],
            Recipe::Empty => Vec::new(),
        }
    }

    pub fn changed(&self) -> usize {
        usize::from(matches!(
            self.recipe,
            Recipe::NullNew | Recipe::NullExisting | Recipe::PresentNull
        ))
    }

    pub fn matched(&self) -> usize {
        self.updates().len()
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = original_rows("Single");
        for (row, _, value) in self.updates() {
            rows[row][1] = value;
        }
        rows
    }

    pub fn sql(&self) -> &'static str {
        match self.recipe {
            Recipe::NullNew => "UPDATE [Single] SET [Left]=30 WHERE ID=3",
            Recipe::NullExisting => "UPDATE [Single] SET [Left]=10 WHERE ID=3",
            Recipe::PresentNull => "UPDATE [Single] SET [Left]=NULL WHERE ID=1",
            Recipe::RepresentedNoop => "UPDATE [Single] SET [Left]=10 WHERE ID=1",
            Recipe::OmittedNoop => "UPDATE [Single] SET [Left]=NULL WHERE ID=3",
            Recipe::Empty => "UPDATE [Single] SET [Left]=30 WHERE ID=99",
        }
    }

    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        super::original_fixed_wire::single_column_original_wire(
            "Single",
            "Left",
            &self.updates(),
            self.before.len(),
            binding,
        )
    }

    pub fn index_rows(&self, alias: &str) -> Vec<Vec<Value>> {
        let rows = self.rows();
        let mut selected: Vec<_> = rows.iter().enumerate().collect();
        match alias {
            "PrimaryID" | "PrimaryAlias" => {}
            "ValueOrder" | "ValueAlias" => {
                selected.retain(|(_, row)| row[1] != Value::Null);
                selected.sort_by_key(|(ordinal, row)| {
                    let Value::Int32(value) = row[1] else {
                        panic!("only present I32 keys are emitted")
                    };
                    (value, *ordinal)
                });
            }
            _ => panic!("outside held four-alias inventory"),
        }
        selected.into_iter().map(|(_, row)| row.clone()).collect()
    }

    pub fn changed_pages(&self) -> Vec<u32> {
        let mut pages = if self.changed() == 0 {
            Vec::new()
        } else {
            vec![self.data, self.roots[1]]
        };
        if self.recipe == Recipe::NullNew {
            pages.push(self.definition);
        }
        pages.sort_unstable();
        let actual: Vec<_> = self
            .before
            .as_chunks::<4096>()
            .0
            .iter()
            .zip(self.after.as_chunks::<4096>().0)
            .enumerate()
            .filter_map(|(page, (before, after))| (before != after).then_some(page as u32))
            .collect();
        assert_eq!(actual, pages);
        pages
    }

    pub fn assert_preserved(&self) {
        let mut expected = self.before.clone();
        let data = self.data as usize * 4096;
        for (row, _, value) in self.updates() {
            let record = data + 4096 - (row + 1) * 19;
            assert_eq!(&self.before[record..record + 2], &[4, 0]);
            match value {
                Value::Int32(value) => {
                    expected[record + 6..record + 10].copy_from_slice(&value.to_le_bytes());
                    expected[record + 18] |= 2;
                }
                Value::Null => expected[record + 18] &= !2,
                _ => panic!("outside held exact I32/NULL assignments"),
            }
        }
        if self.changed() != 0 {
            let mut keys: Vec<_> = self
                .rows()
                .iter()
                .enumerate()
                .filter_map(|(slot, row)| {
                    let Value::Int32(value) = row[1] else {
                        return None;
                    };
                    let mut key = vec![0x7f];
                    key.extend_from_slice(&((value as u32) ^ 0x8000_0000).to_be_bytes());
                    key.extend_from_slice(&((self.data << 8) | slot as u32).to_be_bytes());
                    Some(key)
                })
                .collect();
            keys.sort_unstable();
            let root = self.roots[1] as usize * 4096;
            expected[root + 2..root + 4]
                .copy_from_slice(&((3616 - 9 * keys.len()) as u16).to_le_bytes());
            expected[root + 27..root + 480].fill(0);
            for (position, key) in keys.iter().enumerate() {
                let at = root + 480 + 9 * position;
                expected[at..at + 9].copy_from_slice(key);
                let end = 9 * (position + 1);
                expected[root + 27 + end / 8] |= 1 << (end % 8);
            }
            if self.recipe == Recipe::NullNew {
                let at = self.definition as usize * 4096 + 79;
                expected[at..at + 4].copy_from_slice(&3_u32.to_le_bytes());
            }
        }
        // This comparison includes every unselected table, primary root,
        // historical byte, map, vacated tail, inactive cell and original EOF.
        assert_eq!(expected, self.after);
        let omitted = data + 4096 - 3 * 19;
        if !matches!(self.recipe, Recipe::NullNew | Recipe::NullExisting) {
            assert_eq!(
                &self.after[omitted + 6..omitted + 10],
                &10_i32.to_le_bytes()
            );
        }
        self.changed_pages();
    }

    pub fn forged_offsets(&self) -> Vec<usize> {
        let td = self.definition as usize * 4096;
        let data = self.data as usize * 4096;
        let primary = self.roots[0] as usize * 4096;
        let secondary = self.roots[1] as usize * 4096;
        let mut offsets = vec![
            20,
            td + 16,
            td + 20,
            td + 67,
            td + 79,
            data + 2,
            data + 12,
            data + 14,
            data + 4079,
            data + 4083,
            data + 4087,
            data + 4091,
            data + 4095,
            data + 4045,
            data + 4057,
            primary + 20,
            primary + 480,
            primary + 516,
            secondary + 2,
            secondary + 27,
            secondary + 480,
            secondary + 516,
        ];
        for (page, slot) in std::iter::once((1_usize, 0_usize))
            .chain((0..4).map(|slot| (self.definition as usize + 1, slot)))
        {
            let base = page * 4096;
            let word = base + 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                self.before[word..word + 2].try_into().unwrap(),
            ));
            let end = if slot == 0 {
                4096
            } else {
                usize::from(u16::from_le_bytes(
                    self.before[word - 2..word].try_into().unwrap(),
                ))
            };
            assert_eq!(end - start, 69);
            offsets.push(base + start + 5);
        }
        offsets.sort_unstable();
        offsets.dedup();
        assert_eq!(offsets.len(), 27);
        offsets
    }
}

impl Refusal {
    pub fn table(self) -> &'static str {
        match self {
            Self::Required => "RequiredSingle",
            Self::Unique => "UniqueSingle",
            _ => "Single",
        }
    }

    pub fn updates(self) -> Vec<(usize, Value, Value)> {
        match self {
            Self::Required => vec![(0, Value::Int32(10), Value::Null)],
            Self::Unique => vec![(2, Value::Null, Value::Int32(10))],
            Self::RemovedForward => vec![
                (0, Value::Int32(10), Value::Null),
                (1, Value::Int32(10), Value::Null),
                (2, Value::Null, Value::Int32(10)),
            ],
            Self::RemovedReverse => vec![
                (2, Value::Null, Value::Int32(10)),
                (0, Value::Int32(10), Value::Null),
                (1, Value::Int32(10), Value::Null),
            ],
            Self::CointroducedForward => vec![
                (0, Value::Int32(10), Value::Int32(30)),
                (2, Value::Null, Value::Int32(30)),
            ],
            Self::CointroducedReverse => vec![
                (2, Value::Null, Value::Int32(30)),
                (0, Value::Int32(10), Value::Int32(30)),
            ],
        }
    }

    pub fn sql(self) -> Option<&'static str> {
        match self {
            Self::Required => Some("UPDATE RequiredSingle SET [Left]=NULL WHERE ID=1"),
            Self::Unique => Some("UPDATE UniqueSingle SET [Left]=10 WHERE ID=3"),
            _ => None,
        }
    }
}

pub fn original_rows(table: &str) -> Vec<Vec<Value>> {
    let cells = match table {
        "Single" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(10), Some(200), Some(1002)],
            [Some(3), None, Some(300), Some(1003)],
            [Some(4), Some(20), None, Some(1004)],
        ],
        "UniqueSingle" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(20), Some(200), Some(1002)],
            [Some(3), None, Some(300), Some(1003)],
            [Some(4), None, None, Some(1004)],
        ],
        "RequiredSingle" => vec![
            [Some(1), Some(10), Some(100), Some(1001)],
            [Some(2), Some(20), None, Some(1002)],
        ],
        _ => panic!("outside held scalar source tables"),
    };
    cells
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|cell| cell.map(Value::Int32).unwrap_or(Value::Null))
                .collect()
        })
        .collect()
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let original_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_IGNORE_NULL_UPDATE_ORIGINAL_DIR").unwrap());
    let after_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_IGNORE_NULL_UPDATE_AFTER_DIR").unwrap());
    assert!(original_dir.is_absolute() && after_dir.is_absolute());
    let mut images = Vec::new();
    for (header, format, length, pins) in [
        (
            5,
            "V2016",
            532480,
            [
                "74f6ac1c99b0244a69427afb379112c7321690b9ba628417b1d9411bbb232258",
                "e60cc6d4b66e7b3fb6ee4e6a80295215d1d332e332524d12a43545e02f7b7b2f",
                "cb44abb8dbddc974d27380621a8836170d3871d4d405bb97a511902af7cc6cfb",
                "47b11f15cc98000e675293a37726387dd3862a34781ea7ee139dceba07e22df4",
            ],
        ),
        (
            6,
            "V2019",
            602112,
            [
                "86e518e9d7c9d2b77d72c0457d369748924045c05103aaf6be472f7eecd8bd1a",
                "21a9711a6fa39094c99c7191ad32a4dfd6a74d97f40515d0eefc32ff75513974",
                "8f44acb6268c3955b3c0d6fba5ba018ea2a6ec6872605bdcd5ced2b70babd9e4",
                "9f9626dd04100e3b0829b55db458c9c7f877db219efe10f5b69180a8f9a29969",
            ],
        ),
    ] {
        let mut views = Vec::new();
        for (position, recipe) in [
            "source",
            "single-null-new-expected",
            "single-null-existing-expected",
            "single-present-null-expected",
        ]
        .into_iter()
        .enumerate()
        {
            let directory = if position == 0 {
                &original_dir
            } else {
                &after_dir
            };
            let raw = fs::read(directory.join(format!("{format}-{recipe}.accdb"))).unwrap();
            assert_eq!(raw.len(), length);
            assert_eq!(raw[20], header);
            verify(&raw, pins[position]);
            views.push(raw);
        }
        images.push((header, views));
    }
    let mut cases = Vec::new();
    for (header, views) in images {
        let (definition, data, roots) = match header {
            5 => (89, 93, [91, 92]),
            6 => (106, 110, [108, 109]),
            _ => unreachable!(),
        };
        for (recipe, view) in [
            (Recipe::NullNew, 1),
            (Recipe::NullExisting, 2),
            (Recipe::PresentNull, 3),
            (Recipe::RepresentedNoop, 0),
            (Recipe::OmittedNoop, 0),
            (Recipe::Empty, 0),
        ] {
            let case = Case {
                header,
                recipe,
                before: views[0].clone(),
                after: views[view].clone(),
                definition,
                data,
                roots,
            };
            case.assert_preserved();
            cases.push(case);
        }
    }
    cases
}
