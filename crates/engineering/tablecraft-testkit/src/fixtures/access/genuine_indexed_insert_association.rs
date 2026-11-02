//! Pure hash-pinned source loader; no engine, mutation or oracle dependencies.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub definition: usize,
    pub early: u32,
    pub late: u32,
    pub roots: [usize; 2],
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub fn values(second: bool) -> Vec<Value> {
    let (id, score, text) = if second {
        (11, 18, "new".to_owned())
    } else {
        (10, 17, "L".repeat(200))
    };
    vec![
        Value::Int32(id),
        Value::Int32(score),
        Value::Int32(7),
        Value::Text(text),
    ]
}

fn row(id: i32) -> Vec<Value> {
    match id {
        10 => values(false),
        11 => values(true),
        _ => vec![
            Value::Int32(id),
            Value::Int32(id - 5),
            Value::Int32((id - 1) % 3),
            Value::Text("o".repeat(230)),
        ],
    }
}

pub fn rows(changed: bool) -> Vec<Vec<Value>> {
    let ids = if changed {
        vec![1, 2, 3, 4, 5, 6, 7, 8, 11, 9, 10]
    } else {
        (1..=9).collect()
    };
    ids.into_iter().map(row).collect()
}

pub fn cursor_rows(index: &str) -> Vec<Vec<Value>> {
    let ids = if index.starts_with("Primary") {
        (1..=11).collect::<Vec<_>>()
    } else {
        vec![1, 4, 7, 2, 5, 8, 3, 6, 9, 11, 10]
    };
    ids.into_iter().map(row).collect()
}

/// All four complete source images are verified before any caller can plan.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(env::var_os("TABLECRAFT_INDEXED_INSERT_ASSOCIATION_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, definition, early, late, roots, length, before_sha, after_sha) in [
        (
            5,
            89,
            93,
            95,
            [91, 92],
            409600,
            "0356754e5fa7ea5ea9040e5112b17c6cec46faff388bda9f65d92d34a4864df5",
            "0562fa2e9c3e63ce6667dd2a87bee70867f8ff7956b3a7b8377d33358b666106",
        ),
        (
            6,
            106,
            110,
            112,
            [108, 109],
            479232,
            "abf41a6e18375c84e0f812d682f26ed903145f586a2ee82b6311a215bf2d95d9",
            "7353edf312bddd04edea4f40d375fa3d4789805bcd3a154ea4cebcdf0f43feed",
        ),
    ] {
        let mut read = |suffix: &str, digest: &str| {
            let raw =
                fs::read(directory.join(format!("h{header}-association-{suffix}.accdb"))).unwrap();
            assert_eq!(raw.len(), length);
            verify(&raw, digest);
            raw
        };
        let before = read("original", before_sha);
        let after = read("inserted", after_sha);
        result.push(Case {
            header,
            definition,
            early,
            late,
            roots,
            before,
            after,
        });
    }
    result
}
