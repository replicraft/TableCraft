//! Frozen original-only EOF allocation images; no native writer builds expectations.
use std::{env, fs, path::PathBuf};

pub const OLD: &str = "unchanged sentinel";
pub const GROWN: &str = "aaaaaaaaaaaaaaaaaaa";

pub struct Case {
    pub header: u8,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub root: u32,
    pub target: u32,
}

/// Verify all four independently read image pins before returning any case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_EOF_ALLOCATION_EXPECTED_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, before_sha, after_sha, root, target) in [
        (
            5_u8,
            458752,
            "11029803d999803afd9c5311d1e09788c9209a2aa9f9e6ee23d10fe9920b068e",
            "3a95a4ecc876ea3cc88990cd83309a051515ddad1b698414cb6c51b53d5841db",
            110,
            112,
        ),
        (
            6_u8,
            528384,
            "67ff2be8acf40c287f8281e9044c8eec3296ecab23b9197c2b5d81d790d6f5d0",
            "0db9f897700066ef36bf9acfc90f3f29eda234012ee09515a876bdea3f698b50",
            127,
            129,
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-eof-allocation-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-eof-allocation-growth.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length + 4096);
        verify(&before, before_sha);
        verify(&after, after_sha);
        cases.push(Case {
            header,
            before,
            after,
            root,
            target,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
