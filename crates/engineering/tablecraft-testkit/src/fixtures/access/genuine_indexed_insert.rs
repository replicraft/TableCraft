//! Frozen original-only indexed INSERT source, independently read before writes.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 2],
    pub before: Vec<u8>,
    pub single: Vec<u8>,
    pub batch: Vec<u8>,
}

pub fn values(second: bool) -> Vec<Value> {
    if second {
        vec![Value::Int32(5), Value::Int32(18), Value::Int32(3)]
    } else {
        vec![Value::Int32(4), Value::Int32(17), Value::Int32(3)]
    }
}

pub fn rows(inserted: usize) -> Vec<Vec<Value>> {
    assert!(inserted <= 2);
    let mut result = [[1, -1, 0], [2, 0, 1], [3, 1, 2]]
        .into_iter()
        .map(|row| row.into_iter().map(Value::Int32).collect())
        .collect::<Vec<_>>();
    if inserted > 0 {
        result.push(values(false));
    }
    if inserted > 1 {
        result.push(values(true));
    }
    result
}

/// Verify all six source images before returning any consumer input.
/// Pins are from the first pure manifest 065b10cf..., before matching planners.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_INDEXED_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, definition, data, roots, length, before_sha, single_sha, batch_sha) in [
        (
            5,
            89,
            93,
            [91, 92],
            405504,
            "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
            "92606a16843f6abdc8c1bd286029f24b315e032a2008d657521e12cc5cfb1d77",
            "6a8b5ed907e34714c15cbf4b42521906badfb6492b23f749766e6067201b33e9",
        ),
        (
            6,
            106,
            110,
            [108, 109],
            475136,
            "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
            "95774f6b7b566ece2b7273f506ccfe9a68d22f4d2069f60dc01871c0a6ad15ae",
            "08d29f4c29ebb086989440a23051890520c85a09c82b52faf2d3db27503030bb",
        ),
    ] {
        let mut read = |suffix: &str, digest: &str| {
            let raw = fs::read(directory.join(format!("h{header}-indexed-insert-{suffix}.accdb")))
                .unwrap();
            assert_eq!(raw.len(), length);
            verify(&raw, digest);
            raw
        };
        let before = read("original", before_sha);
        let single = read("single", single_sha);
        let batch = read("shared-secondary-batch", batch_sha);
        cases.push(Case {
            header,
            definition,
            data,
            roots,
            before,
            single,
            batch,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
