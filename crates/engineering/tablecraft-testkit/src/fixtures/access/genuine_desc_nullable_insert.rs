//! SDK/native-qualified DESC/NULL INSERT literals; no engine or SHA dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub before: Vec<u8>,
    pub single: Vec<u8>,
    pub batch: Vec<u8>,
    pub definition: usize,
    pub data: usize,
    pub roots: [usize; 2],
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_DESC_NULL_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "29766255e68f64a0ea63f3ca562d4ba916600fe29eb393020380179faa0d5432",
    );
    let mut cases = Vec::new();
    for (header, length, definition, data, roots, hashes) in [
        (
            5,
            389120,
            89,
            93,
            [91, 92],
            [
                "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
                "4997ebef2b58eabf6142c2785bcc8f57a226322329e8d028d926e6e20fa0842f",
                "9f629f2f43af20c6236490c4cddbdd64eae3ca5660b92b7b658222691136851d",
            ],
        ),
        (
            6,
            458752,
            106,
            110,
            [108, 109],
            [
                "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
                "d8af8c062e4669908c0bab89af46c7f4bf9eda3eb1f56c20404f313c5fa65b85",
                "e41537a9154eeca8fb37d0d221d24420a084866a065573c36e81068f7a78133a",
            ],
        ),
    ] {
        let mut load = |recipe: &str, expected_hash: &str| {
            let bytes =
                fs::read(directory.join(format!("h{header}-desc-null-insert-{recipe}.accdb")))
                    .unwrap();
            assert_eq!(bytes.len(), length);
            verify(&bytes, expected_hash);
            bytes
        };
        cases.push(Case {
            before: load("original", hashes[0]),
            single: load("single-null", hashes[1]),
            batch: load("shared-present-batch", hashes[2]),
            definition,
            data,
            roots,
        });
    }
    cases
}

pub fn values(second: bool) -> Vec<Value> {
    if second {
        vec![Value::Int32(5), Value::Int32(18), Value::Int32(1)]
    } else {
        vec![Value::Int32(4), Value::Int32(17), Value::Null]
    }
}

pub fn rows(count: usize, descending_guard: bool) -> Vec<Vec<Value>> {
    assert!(count <= 2);
    let mut rows = vec![
        vec![Value::Int32(1), Value::Int32(-1), Value::Null],
        vec![Value::Int32(2), Value::Int32(0), Value::Int32(1)],
        vec![Value::Int32(3), Value::Int32(1), Value::Int32(2)],
    ];
    for position in 0..count {
        rows.push(values(position == 1));
    }
    let order = if descending_guard {
        match count {
            0 => vec![2, 1, 0],
            1 => vec![2, 1, 0, 3],
            2 => vec![2, 1, 4, 0, 3],
            _ => unreachable!(),
        }
    } else {
        (0..rows.len()).collect()
    };
    order.into_iter().map(|slot| rows[slot].clone()).collect()
}
