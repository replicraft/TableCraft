//! Frozen independent retained-NODE insertion sources; no engine dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub roots: [usize; 2],
    pub left: [usize; 2],
    pub right: [usize; 2],
    pub data: usize,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(env::var_os("TABLECRAFT_RETAINED_NODE_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, bytes, definition, roots, left, right, data, before_hash, after_hash) in [
        (
            5,
            434176,
            89,
            [91, 92],
            [98, 101],
            [97, 100],
            99,
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            "38f15336e6810158e6fd6b67b5cf1d12ab1c4869fc7d233becc13a12097e1320",
        ),
        (
            6,
            503808,
            106,
            [108, 109],
            [115, 118],
            [114, 117],
            116,
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            "d71fe690d393d1f0a5f5d2256976b452e408cc93cbbbf760fe46c83f2a97144a",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-retained-node-insert-before.accdb")))
                .unwrap();
        let after = fs::read(directory.join(format!("h{header}-retained-node-insert-after.accdb")))
            .unwrap();
        assert_eq!(before.len(), bytes);
        assert_eq!(after.len(), bytes);
        verify(&before, before_hash);
        verify(&after, after_hash);
        result.push(Case {
            before,
            after,
            definition,
            roots,
            left,
            right,
            data,
        });
    }
    result
}

pub fn values() -> Vec<Value> {
    vec![Value::Int32(801), Value::Int32(400), Value::Int32(1)]
}

pub fn rows(inserted: bool, by_guard: bool) -> Vec<Vec<Value>> {
    let mut positions = (0..800_i32).collect::<Vec<_>>();
    if inserted {
        positions.push(800);
    }
    if by_guard {
        positions.sort_unstable_by_key(|position| {
            let guard = if *position == 800 { 1 } else { position % 7 };
            (guard, *position)
        });
    }
    positions
        .into_iter()
        .map(|position| {
            if position == 800 {
                values()
            } else {
                vec![
                    Value::Int32(position + 1),
                    Value::Int32(position - 400),
                    Value::Int32(position % 7),
                ]
            }
        })
        .collect()
}
