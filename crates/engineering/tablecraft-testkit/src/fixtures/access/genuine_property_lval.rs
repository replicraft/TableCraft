//! Explicit retained original/expected files; no engine or production serializer.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

const OLD: &str = "unchanged sentinel";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Same,
    Shorter,
    Empty,
    Clear,
    Noop,
    EmptyGroup,
    Overflow,
    Stale,
}

pub const POSITIVES: [Recipe; 6] = [
    Recipe::Same,
    Recipe::Shorter,
    Recipe::Empty,
    Recipe::Clear,
    Recipe::Noop,
    Recipe::EmptyGroup,
];
pub const REFUSALS: [Recipe; 2] = [Recipe::Overflow, Recipe::Stale];

impl Recipe {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::Shorter => "shorter",
            Self::Empty => "empty",
            Self::Clear => "clear",
            Self::Noop => "noop",
            Self::EmptyGroup => "empty-group",
            Self::Overflow => "overflow",
            Self::Stale => "stale",
        }
    }
}

pub struct Case {
    pub header: u8,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Option<Vec<u8>>,
    pub expected: Value,
    pub replacement: Value,
    pub empty: bool,
    pub changed_rows: usize,
    pub data: u32,
    pub property_offset: usize,
}

fn after_digest(header: u8, recipe: Recipe) -> &'static str {
    match (header, recipe) {
        (5, Recipe::Same) => "298da3b59cd00d6c98bcca4732e3de1bedf355765830d6e208b2310afbc4c188",
        (5, Recipe::Shorter) => "df3aedfb837692b3d520069c4fabda444032c2b21702fd382c76c03574908475",
        (5, Recipe::Empty) => "07d96192142a57b79c2b49303e9b251c1a257b72c3ea34b8d7f002d6b4e86acf",
        (5, Recipe::Clear) => "36f922359e3cc9da4a650b95774a98c863c66e1bf3029926b5a17ffd140518aa",
        (5, Recipe::Noop) => "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        (5, Recipe::EmptyGroup) => {
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4"
        }
        (6, Recipe::Same) => "381bc8dc2ff3491d2c2da093083f199b93cdf4b7e613299e834b7d6157530bc8",
        (6, Recipe::Shorter) => "6adfd7e0effcab5c80243a6ea28414b8ee65c9b8185ac923346fb57037746585",
        (6, Recipe::Empty) => "43b3714d6b7e76267c7c75bbfc1218d7a1c050e06cba93f320ae558bd0efc595",
        (6, Recipe::Clear) => "a3352ae30dc9b6bc4d63877a3d836a0b70d128adb653a3b4de2a3cccce67b3eb",
        (6, Recipe::Noop) => "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        (6, Recipe::EmptyGroup) => {
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d"
        }
        _ => panic!("only twelve frozen positive files exist"),
    }
}

fn request(recipe: Recipe) -> (Value, Value, bool, usize) {
    let expected = Value::Text(if recipe == Recipe::Stale {
        "changed sentinel".into()
    } else {
        OLD.into()
    });
    let replacement = match recipe {
        Recipe::Same | Recipe::Stale => Value::Text("qualified sentinel".into()),
        Recipe::Shorter => Value::Text("changed sentinel".into()),
        Recipe::Empty => Value::Text(String::new()),
        Recipe::Clear => Value::Null,
        Recipe::Noop | Recipe::EmptyGroup => Value::Text(OLD.into()),
        Recipe::Overflow => Value::Text("aaaaaaaaaaaaaaaaaaa".into()),
    };
    let changed = usize::from(matches!(
        recipe,
        Recipe::Same | Recipe::Shorter | Recipe::Empty | Recipe::Clear
    ));
    (expected, replacement, recipe == Recipe::EmptyGroup, changed)
}

/// Verify both originals and all twelve literal AFTER files before returning any
/// case. Callers supply their existing SHA256 implementation; this owner stays
/// std/core-only and never opens a target or asks an engine for expected bytes.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let originals = PathBuf::from(env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected = PathBuf::from(env::var_os("TABLECRAFT_PROPERTY_LVAL_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut profiles = Vec::new();
    for (header, length, digest, data, property_offset) in [
        (
            5_u8,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            110_u32,
            450474_usize,
        ),
        (
            6_u8,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            127_u32,
            470685_usize,
        ),
    ] {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        verify(&before, digest);
        assert_eq!(&before[20..24], &u32::from(header).to_le_bytes());
        let mut afters = Vec::new();
        for recipe in POSITIVES {
            let after =
                fs::read(expected.join(format!("h{header}-marker-{}.accdb", recipe.id()))).unwrap();
            assert_eq!(after.len(), length);
            verify(&after, after_digest(header, recipe));
            afters.push((recipe, after));
        }
        profiles.push((header, before, afters, data, property_offset));
    }

    let mut cases = Vec::new();
    for (header, before, afters, data, property_offset) in profiles {
        for (recipe, after) in afters
            .into_iter()
            .map(|(recipe, after)| (recipe, Some(after)))
            .chain(REFUSALS.into_iter().map(|recipe| (recipe, None)))
        {
            let (expected, replacement, empty, changed_rows) = request(recipe);
            cases.push(Case {
                header,
                recipe,
                before: before.clone(),
                after,
                expected,
                replacement,
                empty,
                changed_rows,
                data,
                property_offset,
            });
        }
    }
    assert_eq!(cases.len(), 16);
    cases
}
