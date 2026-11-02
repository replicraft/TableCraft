//! Independent original-row EOF INSERT sources; no engine or writer dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub batch: bool,
    pub definition: usize,
    pub root: usize,
    pub maps: usize,
    pub bitmap: usize,
    pub eof: usize,
}

/// Verify all eight pure-model/native/SDK-qualified image pins before return.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let mut cases = Vec::new();
    for (variable, batch, entries) in [
        (
            "TABLECRAFT_NORMAL_INSERT_EOF_SOURCE_DIR",
            true,
            [
                (
                    5,
                    458752,
                    107,
                    110,
                    108,
                    111,
                    112,
                    "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
                    "6d01ee69441a765e2dfe28c4affcf9831b90f21b6839ecb582e7232ac6d80cc1",
                ),
                (
                    6,
                    528384,
                    125,
                    127,
                    126,
                    128,
                    129,
                    "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
                    "316adbe11b900e8abd26cbf385a738770622ea39b41b4181f31de43c243fc3e7",
                ),
            ],
        ),
        (
            "TABLECRAFT_NORMAL_SINGLE_INSERT_EOF_SOURCE_DIR",
            false,
            [
                (
                    5,
                    458752,
                    107,
                    110,
                    108,
                    111,
                    112,
                    "9934f7d78854336521bfed4375712fd8ffad283855ae14ee8b3d833c681ff3fa",
                    "91bfe1d2332e2173f88810fea2053ee1ce531b3f7019692eedc41339b2b12b2e",
                ),
                (
                    6,
                    528384,
                    125,
                    127,
                    126,
                    128,
                    129,
                    "3737bed0bc09ac9aa2f4fb780d5bbf771dbd5134c8f4170e12b82ce84b563272",
                    "3c2e562e8cfc91bf299e9fce6394bddfd9762e5ce25aa05a9d97171072611a4d",
                ),
            ],
        ),
    ] {
        let directory = PathBuf::from(env::var_os(variable).unwrap());
        assert!(directory.is_absolute());
        for (header, length, definition, root, maps, bitmap, eof, before_hash, after_hash) in
            entries
        {
            let prefix = if batch {
                format!("h{header}-insert-eof")
            } else {
                format!("h{header}-single-insert-eof")
            };
            let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
            let after = fs::read(directory.join(format!("{prefix}-after.accdb"))).unwrap();
            assert_eq!(before.len(), length);
            assert_eq!(after.len(), length + 4096);
            verify(&before, before_hash);
            verify(&after, after_hash);
            cases.push(Case {
                before,
                after,
                batch,
                definition,
                root,
                maps,
                bitmap,
                eof,
            });
        }
    }
    assert_eq!(cases.len(), 4);
    cases
}

pub fn values(id: i32) -> Vec<Value> {
    vec![Value::Int32(id), Value::Null, Value::Null]
}

pub fn requests(batch: bool) -> Vec<Vec<Value>> {
    let ids = if batch { 24..=278 } else { 260..=260 };
    ids.map(values).collect()
}

pub fn rows(case: &Case, after: bool) -> Vec<Vec<Value>> {
    let mut rows = vec![vec![
        Value::Int32(23),
        Value::Text("unchanged sentinel".into()),
        Value::Binary(vec![199, 0, 255]),
    ]];
    let last = if case.batch {
        if after { 278 } else { 23 }
    } else if after {
        260
    } else {
        259
    };
    rows.extend((24..=last).map(values));
    rows
}
