//! Frozen independent retained-NODE deletion sources; no engine dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub roots: [usize; 2],
    pub left: [usize; 2],
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(env::var_os("TABLECRAFT_RETAINED_NODE_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, bytes, definition, roots, left, before_hash, after_hash) in [
        (
            5,
            434176,
            89,
            [91, 92],
            [98, 101],
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            "265e86c0aa1deded6031e2d367883044401f6647bf058aee18c075ffccc889b9",
        ),
        (
            6,
            503808,
            106,
            [108, 109],
            [115, 118],
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            "23804bc0cc113d6fe2d8279d88329fd1f2aa80852fbf9c32638296b13dd3fdcd",
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-retained-node-delete-before.accdb")))
                .unwrap();
        let after = fs::read(directory.join(format!("h{header}-retained-node-delete-after.accdb")))
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
        });
    }
    result
}

pub fn record(ordinal: usize) -> Vec<u8> {
    assert!(ordinal < 800);
    let position = ordinal as i32;
    let mut bytes = vec![3, 0];
    for value in [position + 1, position - 400, position % 7] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.push(7);
    bytes
}

pub fn rows(deleted: bool, by_guard: bool) -> Vec<Vec<Value>> {
    let mut positions = (0..800_i32)
        .filter(|position| !deleted || ![301, 353].contains(position))
        .collect::<Vec<_>>();
    if by_guard {
        positions.sort_unstable_by_key(|position| (position % 7, *position));
    }
    positions
        .into_iter()
        .map(|position| {
            vec![
                Value::Int32(position + 1),
                Value::Int32(position - 400),
                Value::Int32(position % 7),
            ]
        })
        .collect()
}
