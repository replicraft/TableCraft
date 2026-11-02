//! Original-only EOF growth and independently authored forwarded DELETE images.
use super::genuine_eof_allocation;
use std::{env, fs, path::PathBuf};

pub struct Case {
    /// Qualified pre-growth original, not a producer-created starting file.
    pub original: Vec<u8>,
    /// Independently qualified enlarged image with one forwarded live row.
    pub before: Vec<u8>,
    /// Literal root tombstone/TD count result, retaining hidden record bytes.
    pub after: Vec<u8>,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let originals = genuine_eof_allocation::load_all(&mut verify);
    let directory = PathBuf::from(env::var_os("TABLECRAFT_FORWARDED_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for original in originals {
        let (before_sha, after_sha) = match original.header {
            5 => (
                "3a95a4ecc876ea3cc88990cd83309a051515ddad1b698414cb6c51b53d5841db",
                "c7fad5fe062f7e99478b02e92d6ff6526aeaca66d052b896ea22cf2c410a120f",
            ),
            6 => (
                "0db9f897700066ef36bf9acfc90f3f29eda234012ee09515a876bdea3f698b50",
                "d18dd7c25e7f6aae4a7ed8715e18928bf7beb9bec96acb547cbb109026d65c9d",
            ),
            _ => panic!("unexpected pinned header"),
        };
        let before = fs::read(directory.join(format!(
            "h{}-forwarded-delete-before.accdb",
            original.header,
        )))
        .unwrap();
        let after = fs::read(directory.join(format!(
            "h{}-forwarded-delete-deleted.accdb",
            original.header,
        )))
        .unwrap();
        verify(&before, before_sha);
        verify(&after, after_sha);
        assert_eq!(before, original.after);
        assert_eq!(after.len(), before.len());
        cases.push(Case {
            original: original.before,
            before,
            after,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
