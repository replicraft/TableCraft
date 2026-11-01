//! Four independently qualified compound images, verified before target creation.
use std::{env, fs, path::PathBuf};

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_COMPOUND_EOF_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, before_sha, after_sha) in [
        (
            5,
            466944,
            "2515f4fe4977005335f5aa68a6375ade70b75ffd0dd9f552fa4e62c249a27888",
            "1c555926c1d10f3ee11b6dfa513e38b1dacd0c878602dd7f21b6b9bb5f516cd3",
        ),
        (
            6,
            536576,
            "cbfaca8d36102e477d3aa7a7b86830169de70b5a64ff6d24b10f2855e81a9bde",
            "2e0050cc028b21b6d5bf940a830bd2fd6e45dfc8a1a4036a7a7fa88e7a563cd2",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-compound-eof-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-compound-eof-growth.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length + 4096);
        verify(&before, before_sha);
        verify(&after, after_sha);
        cases.push(Case { before, after });
    }
    assert_eq!(cases.len(), 2);
    cases
}
