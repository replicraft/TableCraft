//! Std/core-only source pin loader, shared by actual native and host consumers.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
}

pub fn values(recipe: &str) -> Vec<Value> {
    let (memo, blob) = match recipe {
        "ascii" => (Some("new".to_owned()), Some(vec![0, 1, 254, 255])),
        "raw-unicode-nul" => (Some("β\0雪".to_owned()), Some(vec![165, 0, 90])),
        "empty" => (Some(String::new()), Some(Vec::new())),
        "null" => (None, None),
        "memo-null-ole-empty" => (None, Some(Vec::new())),
        "memo-empty-ole-null" => (Some(String::new()), None),
        "compressed-limit" => (Some("a".repeat(62)), Some((0..64).collect())),
        "raw-limit" => (Some("β".repeat(32)), Some((0..64).rev().collect())),
        _ => panic!("outside frozen inline source recipes"),
    };
    vec![
        Value::Int32(2),
        memo.map(Value::Text).unwrap_or(Value::Null),
        blob.map(Value::Binary).unwrap_or(Value::Null),
    ]
}

pub fn rows(recipe: &str) -> Vec<Vec<Value>> {
    vec![
        vec![
            Value::Int32(1),
            Value::Text("old".into()),
            Value::Binary(vec![199, 0, 255]),
        ],
        values(recipe),
    ]
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INLINE_LVAL_INSERT_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    let mut originals = Vec::new();
    let mut cases = Vec::new();
    for (header, recipe, filename, length, hash) in [
        (
            5_u8,
            "original",
            "h5-inline-insert-original.accdb",
            380928_usize,
            "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
        ),
        (
            5_u8,
            "ascii",
            "h5-inline-insert-ascii.accdb",
            380928_usize,
            "1be970bbd7511e3b4de48c325f7bc798c53544cc8031623a9339156fa63c85c4",
        ),
        (
            5_u8,
            "raw-unicode-nul",
            "h5-inline-insert-raw-unicode-nul.accdb",
            380928_usize,
            "0bb649e78cdb33e30618db7d27230c68f9a6702dd0d0c32a3d81bc1b44da1fb3",
        ),
        (
            5_u8,
            "empty",
            "h5-inline-insert-empty.accdb",
            380928_usize,
            "4551ad4e85716cbba3407deeb21da8d9efc44e3932f8cf02e377fda29add5479",
        ),
        (
            5_u8,
            "null",
            "h5-inline-insert-null.accdb",
            380928_usize,
            "2d5e1407721a243ec8410652cf8d966a3e1c3b58b93d22744e00eb0e4e69f105",
        ),
        (
            5_u8,
            "memo-null-ole-empty",
            "h5-inline-insert-memo-null-ole-empty.accdb",
            380928_usize,
            "f895177239753d93f1f280e680f631976f3ff0029f00c27890a0c3f6032580be",
        ),
        (
            5_u8,
            "memo-empty-ole-null",
            "h5-inline-insert-memo-empty-ole-null.accdb",
            380928_usize,
            "3034a22ac1fd3487bfdad937d6737c0f08805fd0c474631044b10652be429093",
        ),
        (
            5_u8,
            "compressed-limit",
            "h5-inline-insert-compressed-limit.accdb",
            380928_usize,
            "0323ffed5fcbc26069fee9ed3d44a6b8077997f15f40dff9835deeb83fe8321a",
        ),
        (
            5_u8,
            "raw-limit",
            "h5-inline-insert-raw-limit.accdb",
            380928_usize,
            "8decd1b29fbd86ac1cdfab1a1b632a57388ff9a6a573a99aaee2c9b8f2b35f78",
        ),
        (
            6_u8,
            "original",
            "h6-inline-insert-original.accdb",
            450560_usize,
            "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
        ),
        (
            6_u8,
            "ascii",
            "h6-inline-insert-ascii.accdb",
            450560_usize,
            "906dfa24fa024d2de995d2fa6f1fd75c80a42a5ccdf13ff5db9ff6ede81b2cb9",
        ),
        (
            6_u8,
            "raw-unicode-nul",
            "h6-inline-insert-raw-unicode-nul.accdb",
            450560_usize,
            "09bc773b55a19b14c1bfa9fb6d2a88569338a588e06e4410a63b07ccc2b7766d",
        ),
        (
            6_u8,
            "empty",
            "h6-inline-insert-empty.accdb",
            450560_usize,
            "9dce1c1035d14cdc8b37779482c561b272a04cbb1a4240399d3906cf492fe06b",
        ),
        (
            6_u8,
            "null",
            "h6-inline-insert-null.accdb",
            450560_usize,
            "d671add198221de944a1863db5dd09aac97a2d4cb1364d7f80782a12a89dd86c",
        ),
        (
            6_u8,
            "memo-null-ole-empty",
            "h6-inline-insert-memo-null-ole-empty.accdb",
            450560_usize,
            "5c5382b3f61b3d1465a9c47b475e16e42185b976405e18ac3b4a54deeb485011",
        ),
        (
            6_u8,
            "memo-empty-ole-null",
            "h6-inline-insert-memo-empty-ole-null.accdb",
            450560_usize,
            "e3a81cdf0439c76fefecdfdd22f90ad8426b61ee1571b8841d8068fcfcfee489",
        ),
        (
            6_u8,
            "compressed-limit",
            "h6-inline-insert-compressed-limit.accdb",
            450560_usize,
            "a6a6384ebae73f49c9cc7fffdfa00f2d365f4c980080cbacaf9ca9c88cade981",
        ),
        (
            6_u8,
            "raw-limit",
            "h6-inline-insert-raw-limit.accdb",
            450560_usize,
            "00aa4de17b9924cc8cd1c6135d76eaa229457de00968eba4313051be458cc538",
        ),
    ] {
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        verify(&raw, hash);
        if recipe == "original" {
            originals.push(raw);
        } else {
            cases.push(Case {
                header,
                recipe,
                before: originals[usize::from(header - 5)].clone(),
                after: raw,
                definition: if header == 5 { 89 } else { 106 },
                data: if header == 5 { 91 } else { 108 },
            });
        }
    }
    assert_eq!(cases.len(), 16);
    cases
}
