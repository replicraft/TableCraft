//! Frozen Binary DELETE source outcomes; no mutation output supplies expectations.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Null,
    Empty,
    OneDuplicate,
    BothDuplicates,
    Segments,
    All,
}

impl Recipe {
    pub fn selected(self) -> &'static [usize] {
        match self {
            Self::Null => &[0],
            Self::Empty => &[1],
            Self::OneDuplicate => &[2],
            Self::BothDuplicates => &[2, 12],
            Self::Segments => &[6, 7, 8, 9],
            Self::All => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            Self::Null => "DELETE FROM BinaryKeys WHERE IsNull([KeyValue])",
            Self::Empty => "DELETE FROM BinaryKeys WHERE [Id]=2",
            Self::OneDuplicate => "DELETE FROM BinaryKeys WHERE [Id]=3",
            Self::BothDuplicates => "DELETE FROM BinaryKeys WHERE [Id]=3 OR [Id]=13",
            Self::Segments => "DELETE FROM BinaryKeys WHERE [Id]>=7 AND [Id]<=10",
            Self::All => "DELETE FROM BinaryKeys",
        }
    }
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

pub fn original_rows() -> Vec<Vec<Value>> {
    super::genuine_binary_indexed::original_rows()
}

pub fn final_rows(recipe: Recipe) -> Vec<Vec<Value>> {
    original_rows()
        .into_iter()
        .enumerate()
        .filter_map(|(slot, row)| (!recipe.selected().contains(&slot)).then_some(row))
        .collect()
}

/// All fourteen hashes are checked before the first case is returned.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_DELETE_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    let before5 = fs::read(dir.join("h5-binary-before.accdb")).unwrap();
    let before6 = fs::read(dir.join("h6-binary-before.accdb")).unwrap();
    verify(
        &before5,
        "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
    );
    verify(
        &before6,
        "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
    );
    let mut cases = Vec::new();
    for (header, recipe, filename, hash) in [
        (
            5_u8,
            Recipe::Null,
            "h5-binary-delete-null.accdb",
            "5db3be2121ac9e6f5ceff8cf0cf2156078d6a0b6907c6ea11872e2330cefa988",
        ),
        (
            5_u8,
            Recipe::Empty,
            "h5-binary-delete-empty.accdb",
            "0406d9549bd03bc4c85a79cfe3645aabdf05cb28de282bec51f3a13e29ada5e8",
        ),
        (
            5_u8,
            Recipe::OneDuplicate,
            "h5-binary-delete-one-duplicate.accdb",
            "7a661d62293d9c6ecbc837d9f2b71a3af43b45f2c656fe25498572624bb8c7f7",
        ),
        (
            5_u8,
            Recipe::BothDuplicates,
            "h5-binary-delete-both-duplicates.accdb",
            "e5303c44e676afb7c8127c5f3ad946a0c5886abcd08f536ab8b064fee1228976",
        ),
        (
            5_u8,
            Recipe::Segments,
            "h5-binary-delete-segments.accdb",
            "8780297c0ae0a370e49cdf03495798e018bcf6ad286d983e865b63f287f09424",
        ),
        (
            5_u8,
            Recipe::All,
            "h5-binary-delete-all.accdb",
            "8bdcae4fec767c8b9720d099d6464be895bf1eadcfef7c8680f1aa4e54f93997",
        ),
        (
            6_u8,
            Recipe::Null,
            "h6-binary-delete-null.accdb",
            "66fa70f2a08c94dd4f63637032ed0a8ecb09df6d28616b906362ef181cd78acf",
        ),
        (
            6_u8,
            Recipe::Empty,
            "h6-binary-delete-empty.accdb",
            "ce11d6e37a07de416f6aa44be8cb5a20d4a1515f615d1dc4350a82d915e5789b",
        ),
        (
            6_u8,
            Recipe::OneDuplicate,
            "h6-binary-delete-one-duplicate.accdb",
            "f07291b02758943761eee2b305af722e354de6fd5f3d7f8c9dd9b248e91dad4f",
        ),
        (
            6_u8,
            Recipe::BothDuplicates,
            "h6-binary-delete-both-duplicates.accdb",
            "5bfa53e54edc59e3b262b68c0bc1011e3dc20b082eab04b0a421556768022893",
        ),
        (
            6_u8,
            Recipe::Segments,
            "h6-binary-delete-segments.accdb",
            "76e3537befe14e5088beb700ba3d5b4c6124f9fe70a54b220e204968372c56ff",
        ),
        (
            6_u8,
            Recipe::All,
            "h6-binary-delete-all.accdb",
            "aafffecbb1e4a7e6607cac67a6ac9e4f7b97a57cd52747f01f358ba01cb28401",
        ),
    ] {
        let after = fs::read(dir.join(filename)).unwrap();
        verify(&after, hash);
        let (before, definition, data, roots) = if header == 5 {
            (&before5, 89, 94, [91, 92, 93])
        } else {
            (&before6, 106, 111, [108, 109, 110])
        };
        assert_eq!(after.len(), before.len());
        cases.push(Case {
            header,
            recipe,
            before: before.clone(),
            after,
            definition,
            data,
            roots,
        });
    }
    assert_eq!(cases.len(), 12);
    cases
}
