//! SDK originals plus independent pre-producer root-split model images.
use std::{fs, path::PathBuf};

pub struct Case {
    pub header: u8,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub root: usize,
    pub data: usize,
    pub eof: usize,
}

/// Pin all four source files before returning cases to any matching producer.
/// The caller supplies SHA-256; this std-only owner adds no hash dependency.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_ROOT_SPLIT_REPOSITORY").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (profile, header, before_hash, after_hash, definition, root, data, eof) in [
        (
            "V2016",
            5,
            "3226fb48824cae7bf9de03da58df2946c58acd239d33c6604c78a0a5473e6932",
            "d8ade2dbaa5ce075f95b08e8c801a07864221cb9772eab8de95ed44dee418d60",
            89,
            91,
            94,
            95,
        ),
        (
            "V2019",
            6,
            "00badd8da31daedd3b1ce3637d334e251158fd2fbd7e02f264860c771e4aa489",
            "9117623259fa78d90fe155aec29debbf004d10d77fe5ff8b146d3b950d9e230c",
            106,
            108,
            112,
            113,
        ),
    ] {
        let before = fs::read(directory.join(format!(
            ".cache/ace16-insert-root-split-reference-first-20261027/{profile}-fitted-source.accdb"
        )))
        .unwrap();
        let after = fs::read(directory.join(format!(".cache/ace16-insert-root-split-independent-next-20261027/expected/{profile}-fitted.expected.bin"))).unwrap();
        verify(&before, before_hash);
        verify(&after, after_hash);
        assert_eq!(before.len(), eof * 4096);
        assert_eq!(after.len(), (eof + 2) * 4096);
        assert_eq!(before[20], header);
        assert_eq!(after[20], header);
        cases.push(Case {
            header,
            before,
            after,
            definition,
            root,
            data,
            eof,
        });
    }
    cases
}
