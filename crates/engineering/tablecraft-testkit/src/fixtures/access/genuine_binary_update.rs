//! Original-only pins and source-frozen fitting Binary UPDATE requests.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const VIEWS: &[(u8, &str, &str, &str)] = &[
    (
        5,
        "before",
        "h5-binary-before.accdb",
        "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
    ),
    (
        5,
        "same-length-new-family",
        "h5-binary-same-length-new-family.accdb",
        "939eb9a6167e340977724afe6ddca2832aa987f399f090fef4a7aaccba9e898d",
    ),
    (
        5,
        "shrink-to-existing",
        "h5-binary-shrink-to-existing.accdb",
        "d518076de0bd3f7fe89aa09350573c2098339cc2ff696ca2d5a460e86fd290a0",
    ),
    (
        5,
        "null-to-empty",
        "h5-binary-null-to-empty.accdb",
        "f30b3a6387f35803b818741d2557dcb5754c7f8a81d3b1f6399a37e1e0bbf07d",
    ),
    (
        5,
        "empty-to-null",
        "h5-binary-empty-to-null.accdb",
        "f4996b222db99cb5c6ab00c47a35fa82e8f24658ccd1156e492952a02d3994dd",
    ),
    (
        5,
        "duplicate-to-new",
        "h5-binary-duplicate-to-new.accdb",
        "b8113a41d0e6a1969a1bb0319858de03d79d7cf21e0ac03b595026bf619eefc8",
    ),
    (
        5,
        "present-to-null",
        "h5-binary-present-to-null.accdb",
        "897b29f800f2a4c3adaec60a72cf749fd08680e29b15f505ea4f6512fddc41ce",
    ),
    (
        5,
        "duplicate-merge",
        "h5-binary-duplicate-merge.accdb",
        "bebda3b3c8afcb584cf10e84b37e5c7fa0042aed1d291502116c1f0b656d75da",
    ),
    (
        6,
        "before",
        "h6-binary-before.accdb",
        "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
    ),
    (
        6,
        "same-length-new-family",
        "h6-binary-same-length-new-family.accdb",
        "88cb22dfa0b6adbfd3f2f82db4e847f77268c1c802e144b21249da17b1995e1e",
    ),
    (
        6,
        "shrink-to-existing",
        "h6-binary-shrink-to-existing.accdb",
        "4516b34d843dc818877fbc5741532cbb01fae2b8608f201b3449486bd5cc4f51",
    ),
    (
        6,
        "null-to-empty",
        "h6-binary-null-to-empty.accdb",
        "594a04ab95c16524eccaa948fbd4b439fc6ede2d1caaf2d8f1c835b397cce39a",
    ),
    (
        6,
        "empty-to-null",
        "h6-binary-empty-to-null.accdb",
        "ec34b545f5fbc3564edea79e00475a69e19642a9ce0f32ac40f24584b878ec84",
    ),
    (
        6,
        "duplicate-to-new",
        "h6-binary-duplicate-to-new.accdb",
        "03d42fa05039785612520aa19ff9547a1e80808a249a7a7f25fbfb4c207d82c4",
    ),
    (
        6,
        "present-to-null",
        "h6-binary-present-to-null.accdb",
        "ccd681ed67af39beb2d7615b645ef9799775a0cb76c166f528419d78a0fd4395",
    ),
    (
        6,
        "duplicate-merge",
        "h6-binary-duplicate-merge.accdb",
        "5574184b32cccb5c003c86279cf114365c88d5343b8c180fb7efb43424b0dbc6",
    ),
];

pub struct Case {
    pub header: u8,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
}

impl Case {
    pub fn changed_rows(&self) -> usize {
        if self.recipe == "duplicate-merge" {
            2
        } else {
            1
        }
    }
}

pub fn load_all(check: impl Fn(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(
        std::env::var_os("TABLECRAFT_BINARY_UPDATE_SOURCE_DIR")
            .expect("qualified Binary UPDATE source directory"),
    );
    assert!(dir.is_absolute());
    // Every original and AFTER pin is verified before any consumer gets a case.
    let views = VIEWS
        .iter()
        .map(|(header, recipe, filename, hash)| {
            let raw = fs::read(dir.join(filename)).expect("qualified source image");
            check(&raw, hash);
            (*header, *recipe, raw)
        })
        .collect::<Vec<_>>();
    let mut cases = Vec::new();
    for (header, recipe, raw) in &views {
        if *recipe == "before" {
            continue;
        }
        let before = views
            .iter()
            .find(|(old_header, old_recipe, _)| old_header == header && *old_recipe == "before")
            .unwrap()
            .2
            .clone();
        let (definition, data, roots) = if *header == 5 {
            (89, 94, [91, 92, 93])
        } else {
            (106, 111, [108, 109, 110])
        };
        cases.push(Case {
            header: *header,
            recipe,
            before,
            after: raw.clone(),
            definition,
            data,
            roots,
        });
    }
    cases
}

pub fn updates(recipe: &str) -> Vec<(usize, Value, Value)> {
    let binary = |value: &[u8]| Value::Binary(value.to_vec());
    match recipe {
        "same-length-new-family" => vec![(3, binary(&[255]), binary(&[254]))],
        "shrink-to-existing" => vec![(
            9,
            binary(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
            binary(&[0, 1, 2, 3, 4, 5, 6, 7]),
        )],
        "null-to-empty" => vec![(0, Value::Null, binary(&[]))],
        "empty-to-null" => vec![(1, binary(&[]), Value::Null)],
        "duplicate-to-new" => vec![(2, binary(&[0]), binary(&[85]))],
        "present-to-null" => vec![(8, binary(&[0, 1, 2, 3, 4, 5, 6, 7, 8]), Value::Null)],
        "duplicate-merge" => vec![
            (2, binary(&[0]), binary(&[85])),
            (12, binary(&[0]), binary(&[85])),
        ],
        _ => panic!("outside frozen Binary UPDATE requests"),
    }
}

pub fn final_rows(recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = super::genuine_binary_indexed::original_rows();
    for (ordinal, expected, value) in updates(recipe) {
        assert_eq!(rows[ordinal][1], expected);
        rows[ordinal][1] = value;
    }
    rows
}

pub fn sql(recipe: &str) -> Option<&'static str> {
    match recipe {
        "empty-to-null" => Some("UPDATE BinaryKeys SET KeyValue=NULL WHERE Id=2"),
        "present-to-null" => Some("UPDATE BinaryKeys SET KeyValue=NULL WHERE Id=9"),
        _ => None,
    }
}
