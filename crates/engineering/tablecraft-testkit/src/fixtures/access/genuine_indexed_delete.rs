//! Independent genuine originals and literal indexed DELETE images, not a writer oracle.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

const ROWS: [[i32; 3]; 3] = [[1, -1, 0], [2, 0, 1], [3, 1, 2]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    First,
    Middle,
    Exterior,
    All,
}

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Middle => "middle",
            Self::Exterior => "exterior",
            Self::All => "all",
        }
    }

    pub const fn selected(self) -> &'static [usize] {
        match self {
            Self::First => &[0],
            Self::Middle => &[1],
            Self::Exterior => &[0, 2],
            Self::All => &[0, 1, 2],
        }
    }

    pub const fn sql(self) -> &'static str {
        match self {
            Self::First => "DELETE FROM Items WHERE [Id]=1",
            Self::Middle => "DELETE FROM Items WHERE [Id]=2",
            Self::Exterior => "DELETE FROM Items WHERE [Id]=1 OR [Id]=3",
            Self::All => "DELETE FROM Items",
        }
    }
}

pub struct Case {
    pub header: u8,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub data: usize,
    pub roots: [usize; 2],
}

pub fn original_rows() -> Vec<Vec<Value>> {
    ROWS.iter()
        .map(|row| row.iter().copied().map(Value::Int32).collect())
        .collect()
}

pub fn final_rows(recipe: Recipe) -> Vec<Vec<Value>> {
    ROWS.iter()
        .enumerate()
        .filter(|(slot, _)| !recipe.selected().contains(slot))
        .map(|(_, row)| row.iter().copied().map(Value::Int32).collect())
        .collect()
}

/// Explicit original15-byte records, never produced by a native serializer.
pub fn witnesses(recipe: Recipe, reverse: bool) -> Vec<(usize, Vec<u8>)> {
    assert!(!reverse || recipe == Recipe::Exterior);
    let selected = if reverse {
        &[2, 0][..]
    } else {
        recipe.selected()
    };
    selected
        .iter()
        .map(|slot| {
            let mut raw = vec![3, 0];
            for value in ROWS[*slot] {
                raw.extend_from_slice(&value.to_le_bytes());
            }
            raw.push(7);
            assert_eq!(raw.len(), 15);
            (*slot, raw)
        })
        .collect()
}

/// Check the entire16-view source set before returning the first usable case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_INDEXED_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, recipe, after_sha) in [
        (
            5,
            Recipe::First,
            "aca0a0c8d49867e680f6fed07d26e6a9e3c2bafdc9968d7bce30cbe42a9c27b2",
        ),
        (
            5,
            Recipe::Middle,
            "c9fc6bd2a25ea403e5baa3464002fb6f8c2bcd9de44bf1b5ef2fc3091203ae4d",
        ),
        (
            5,
            Recipe::Exterior,
            "9f16d86877ce3e0430bd3d7b3161099f3b51d508932f396b932a94b4ccdb039c",
        ),
        (
            5,
            Recipe::All,
            "1aa761e24fabbab4760086a8271b2075efdeac4e19aeb6c0aad80627877a1885",
        ),
        (
            6,
            Recipe::First,
            "b77fb1b76fdddd9334a95826ad3c3ca367d41c588c151aca38393386bc251824",
        ),
        (
            6,
            Recipe::Middle,
            "90850fe6764b7bc140487658c4f048369fd1c8ad56faa4f0759ea16b7ac02bcc",
        ),
        (
            6,
            Recipe::Exterior,
            "cb3507d404d487af308f77f3ce02f33759d876428f09eac25289caf590ea75c9",
        ),
        (
            6,
            Recipe::All,
            "185e81274647b9f4ef79b51ba4ae830095d553ce2fa0b40bcc26ff00b971cf2e",
        ),
    ] {
        let (definition, data, roots, length, before_sha) = match header {
            5 => (
                89,
                93,
                [91, 92],
                405504,
                "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            ),
            6 => (
                106,
                110,
                [108, 109],
                475136,
                "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            ),
            _ => panic!("unexpected frozen source header"),
        };
        let prefix = format!("h{header}-indexed-delete-{}", recipe.id());
        let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
        let after = fs::read(directory.join(format!("{prefix}-deleted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length);
        verify(&before, before_sha);
        verify(&after, after_sha);
        for (slot, raw) in witnesses(Recipe::All, false) {
            let start = data * 4096 + 4081 - slot * 15;
            assert_eq!(&before[start..start + 15], raw);
            assert_eq!(&after[start..start + 15], raw);
        }
        result.push(Case {
            header,
            recipe,
            before,
            after,
            definition,
            data,
            roots,
        });
    }
    assert_eq!(result.len(), 8);
    result
}
