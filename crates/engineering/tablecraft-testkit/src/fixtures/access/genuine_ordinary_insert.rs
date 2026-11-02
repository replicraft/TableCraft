//! Independently authored INSERT images over the unchanged genuine originals.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub definition: u32,
    pub data: u32,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

/// Complete explicit native-field-order request, with no generated or omitted cells.
pub fn values() -> Vec<Value> {
    vec![Value::Int32(24), Value::Text("new".into()), Value::Null]
}

/// Verify all original and independently constructed image pins before use.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_NORMAL_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, definition, data, length, before_sha, after_sha) in [
        (
            5,
            107,
            110,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "52d9ecd720e1292f325c35f495d4c2db9ef0cdf700154b409f7e017200b4b4c9",
        ),
        (
            6,
            125,
            127,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "81ae4c1a7eaf5b6957b07a4ba24b3e50de5eadf667dc2c03c446dc1d844f8a72",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-ordinary-insert-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-ordinary-insert-inserted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length);
        verify(&before, before_sha);
        verify(&after, after_sha);
        cases.push(Case {
            header,
            definition,
            data,
            before,
            after,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
