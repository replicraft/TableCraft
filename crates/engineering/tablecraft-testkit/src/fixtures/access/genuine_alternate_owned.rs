//! Independently authored pre-owned SOURCE and exact request-derived outcomes.
use std::{env, fs, path::PathBuf};

pub const OLD: &str = "unchanged sentinel";
pub const GROWN: &str = "aaaaaaaaaaaaaaaaaaa";
pub const SECOND: &str = "bbbbbbbbbbbbbbbbbbb";

pub struct Case {
    pub header: u8,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub second: Vec<u8>,
    pub root: u32,
    pub target: u32,
    pub property_offset: usize,
}

/// All six pins must pass before returning a case to a target-owning caller.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let first = PathBuf::from(env::var_os("TABLECRAFT_ALTERNATE_OWNED_EXPECTED_DIR").unwrap());
    let second =
        PathBuf::from(env::var_os("TABLECRAFT_ALTERNATE_OWNED_SECOND_EXPECTED_DIR").unwrap());
    assert!(first.is_absolute() && second.is_absolute());
    let mut cases = Vec::new();
    for (header, length, before_sha, after_sha, second_sha, root, target, property_offset) in [
        (
            5_u8,
            462848,
            "a8954a42742b6414726055970c628c88436059d7e14f919d939cae95d35c4cbd",
            "b08e1cb05999cc30970ea97d8ea4b1bbc105ccb6cd05ccda106618914591645e",
            "1a46e7b1dbc7d4ecb58e72b58843ce96dbc3174298d91a5b395c122b8bf16a32",
            110,
            112,
            450474,
        ),
        (
            6_u8,
            532480,
            "a0b3bf39db8322a21b90cde1c72d168f1ece059c268fae6212eb93c4692b0a21",
            "28ce5406b7fa8b386c4ad6a1060e405c3b7c784d882d0df32f498b1d4b7c97c3",
            "1009ffcf430e01624a8cb0f7bd6ca5edddbe8059e8099260828ce17062cad0d6",
            127,
            129,
            470685,
        ),
    ] {
        let before =
            fs::read(first.join(format!("h{header}-alternate-owned-before.accdb"))).unwrap();
        let after =
            fs::read(first.join(format!("h{header}-alternate-owned-growth.accdb"))).unwrap();
        let second =
            fs::read(second.join(format!("h{header}-alternate-owned-second.accdb"))).unwrap();
        for (raw, expected) in [
            (&before, before_sha),
            (&after, after_sha),
            (&second, second_sha),
        ] {
            assert_eq!(raw.len(), length);
            verify(raw, expected);
        }
        cases.push(Case {
            header,
            before,
            after,
            second,
            root,
            target,
            property_offset,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
