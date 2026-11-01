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
            "69a6dff12414839a8440be4ccc86d43658f14f1315ae0f4461fd236794295d9e",
            "5d3b8b2f5cb5ca065f5837aec5197c7389e7b200e55c005be27c2dcc105c6787",
            89,
            91,
            103,
            104,
        ),
        (
            "V2019",
            6,
            "f695ebe4a18c392c014f49506cd741bac7760bd4b398f83fb4c02925275e2e38",
            "4597231d353a5e669a8ea029d8caeb44a43ffa5e3628e91593f26fcc82991752",
            106,
            108,
            121,
            122,
        ),
    ] {
        let before = fs::read(directory.join(format!(
            ".cache/ace16-insert-root-split-reference-first-20261027/{profile}-full84-source.accdb"
        )))
        .unwrap();
        let after = fs::read(directory.join(format!(".cache/ace16-insert-root-split-independent-next-20261027/expected/{profile}-full84.expected.bin"))).unwrap();
        verify(&before, before_hash);
        verify(&after, after_hash);
        assert_eq!(before.len(), eof * 4096);
        assert_eq!(after.len(), (eof + 3) * 4096);
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
