//! Eagerly pinned external reuse literals, authored before matching producers.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const PINS: &[(&str, &str, &str, usize, &str)] = &[
    (
        "V2016-raw",
        "original",
        "V2016-raw-original.accdb",
        446464,
        "9a6b7e0e5b0984e51e1af3170ff92e641b48809db2f74c17ab634f80993c0359",
    ),
    (
        "V2016-raw",
        "joint-single65-66",
        "V2016-raw-joint-single65-66.accdb",
        446464,
        "e6a086fd5563057e33d62e5f80442d44af46bf1322245d4343269a61e4f94a95",
    ),
    (
        "V2016-raw",
        "joint-single4076",
        "V2016-raw-joint-single4076.accdb",
        446464,
        "2eebb165a4a8d4a358f2149729875d128f855df442bd1f059a631fbc5eaef91c",
    ),
    (
        "V2016-raw",
        "joint-chain4077-4078",
        "V2016-raw-joint-chain4077-4078.accdb",
        446464,
        "eed48ec03b35e4daecc87143a1a56715a43fda66c7fcecc5e594a93fb0a0000d",
    ),
    (
        "V2016-raw",
        "memo-last-tail-shrink",
        "V2016-raw-memo-last-tail-shrink.accdb",
        446464,
        "0b4be1c5367ded978cf8da388f28c78a578a74ca0572a168e7e4a9f821ae0b5e",
    ),
    (
        "V2016-raw",
        "blob-last-slot-shrink",
        "V2016-raw-blob-last-slot-shrink.accdb",
        446464,
        "f83a4aca8948008a7e1ed81d2ef6ac28b7433ff81cc1fde01401b28f9229aab1",
    ),
    (
        "V2016-raw",
        "noop-chain",
        "V2016-raw-noop-chain.accdb",
        446464,
        "9a6b7e0e5b0984e51e1af3170ff92e641b48809db2f74c17ab634f80993c0359",
    ),
    (
        "V2016-raw",
        "empty-group",
        "V2016-raw-empty-group.accdb",
        446464,
        "9a6b7e0e5b0984e51e1af3170ff92e641b48809db2f74c17ab634f80993c0359",
    ),
    (
        "V2016-compressed",
        "original",
        "V2016-compressed-original.accdb",
        446464,
        "1f5c45d48680ef13c474ebe9b6a6a3e4a745ac7dcdca94c5f3b2b96d82d11c04",
    ),
    (
        "V2016-compressed",
        "joint-single65-66",
        "V2016-compressed-joint-single65-66.accdb",
        446464,
        "d8d257b8adead639a7d1d1137a08beac8e362ccd8bff05fde3300d782a91b8a7",
    ),
    (
        "V2016-compressed",
        "joint-single4076",
        "V2016-compressed-joint-single4076.accdb",
        446464,
        "48d9509a6f76c269e1f52801117bf2ac90b310b932a404134dd5829f2aceed9a",
    ),
    (
        "V2016-compressed",
        "joint-chain4077-4078",
        "V2016-compressed-joint-chain4077-4078.accdb",
        446464,
        "cebdb98d781d09a66a0f714eeac6fc6840a009acb403f325ef4861adeae43b30",
    ),
    (
        "V2016-compressed",
        "memo-last-tail-shrink",
        "V2016-compressed-memo-last-tail-shrink.accdb",
        446464,
        "2ec46aad63e9271bdb427ac72bd96b68f24fc3355bc9a809e9503640d75d412c",
    ),
    (
        "V2016-compressed",
        "blob-last-slot-shrink",
        "V2016-compressed-blob-last-slot-shrink.accdb",
        446464,
        "ad4f046095dbd5f27a95bc289a0d4dbb02740ea0dc871ca207b4977d5d1a25f2",
    ),
    (
        "V2016-compressed",
        "noop-chain",
        "V2016-compressed-noop-chain.accdb",
        446464,
        "1f5c45d48680ef13c474ebe9b6a6a3e4a745ac7dcdca94c5f3b2b96d82d11c04",
    ),
    (
        "V2016-compressed",
        "empty-group",
        "V2016-compressed-empty-group.accdb",
        446464,
        "1f5c45d48680ef13c474ebe9b6a6a3e4a745ac7dcdca94c5f3b2b96d82d11c04",
    ),
    (
        "V2019-raw",
        "original",
        "V2019-raw-original.accdb",
        520192,
        "986ddd0db110ef562ead874c3c27dbba514a6d2154cb49bf0836d17f3966f125",
    ),
    (
        "V2019-raw",
        "joint-single65-66",
        "V2019-raw-joint-single65-66.accdb",
        520192,
        "5486248236b94834bed8e3bb8a8df1f37c6ac431607418b290e5abc373486901",
    ),
    (
        "V2019-raw",
        "joint-single4076",
        "V2019-raw-joint-single4076.accdb",
        520192,
        "4e9c3f6746002fe56d857ed74d9bed439256dcc5b92401093ee92e7553e60fd4",
    ),
    (
        "V2019-raw",
        "joint-chain4077-4078",
        "V2019-raw-joint-chain4077-4078.accdb",
        520192,
        "c6c754733bcce6e9ce44706324e3638382d9a5803c7144133900251ecc77ae1a",
    ),
    (
        "V2019-raw",
        "memo-last-tail-shrink",
        "V2019-raw-memo-last-tail-shrink.accdb",
        520192,
        "5d2d78be2e2af8e463c4a239228fc1933364ab1b43e551b742bcfa945ea1663b",
    ),
    (
        "V2019-raw",
        "blob-last-slot-shrink",
        "V2019-raw-blob-last-slot-shrink.accdb",
        520192,
        "c9be5dcb5ceb14e5eda376c4fa4f0ea92203e755732f7c117e1ebcb3c42f6fe6",
    ),
    (
        "V2019-raw",
        "noop-chain",
        "V2019-raw-noop-chain.accdb",
        520192,
        "986ddd0db110ef562ead874c3c27dbba514a6d2154cb49bf0836d17f3966f125",
    ),
    (
        "V2019-raw",
        "empty-group",
        "V2019-raw-empty-group.accdb",
        520192,
        "986ddd0db110ef562ead874c3c27dbba514a6d2154cb49bf0836d17f3966f125",
    ),
    (
        "V2019-compressed",
        "original",
        "V2019-compressed-original.accdb",
        520192,
        "e8c39e4b9e52f4204e5e58338d2180efaba980e193c4be078fc84aaf771454de",
    ),
    (
        "V2019-compressed",
        "joint-single65-66",
        "V2019-compressed-joint-single65-66.accdb",
        520192,
        "b083d5fe176e6011cf1dc8cd3a0948763f8398d61186c39d2a92b5dd3064525c",
    ),
    (
        "V2019-compressed",
        "joint-single4076",
        "V2019-compressed-joint-single4076.accdb",
        520192,
        "42a9cdde5662b5ec7612f365f8f136e57a2c69fd04b8f75a232b12121e062dd0",
    ),
    (
        "V2019-compressed",
        "joint-chain4077-4078",
        "V2019-compressed-joint-chain4077-4078.accdb",
        520192,
        "a57ee3cff8fd59165beaebab1adb68bafa9f00160093de2dec718e8ec253c479",
    ),
    (
        "V2019-compressed",
        "memo-last-tail-shrink",
        "V2019-compressed-memo-last-tail-shrink.accdb",
        520192,
        "e7b13ec81fe9f23795f293cc7d70f93632c2213cb41cc9bae08f6a1a7788759a",
    ),
    (
        "V2019-compressed",
        "blob-last-slot-shrink",
        "V2019-compressed-blob-last-slot-shrink.accdb",
        520192,
        "a0ee398d0945b5fec9290aaec5058fa9e353dae2e6816cdb345e3e404265dfea",
    ),
    (
        "V2019-compressed",
        "noop-chain",
        "V2019-compressed-noop-chain.accdb",
        520192,
        "e8c39e4b9e52f4204e5e58338d2180efaba980e193c4be078fc84aaf771454de",
    ),
    (
        "V2019-compressed",
        "empty-group",
        "V2019-compressed-empty-group.accdb",
        520192,
        "e8c39e4b9e52f4204e5e58338d2180efaba980e193c4be078fc84aaf771454de",
    ),
];

const PROFILES: &[(&str, u8, u32, u32)] = &[
    ("V2016-raw", 5, 89, 93),
    ("V2016-compressed", 5, 89, 93),
    ("V2019-raw", 6, 106, 110),
    ("V2019-compressed", 6, 106, 110),
];

const RECIPES: &[&str] = &[
    "joint-single65-66",
    "joint-single4076",
    "joint-chain4077-4078",
    "memo-last-tail-shrink",
    "blob-last-slot-shrink",
    "noop-chain",
    "empty-group",
];

pub struct Update {
    pub row: usize,
    pub expected: Value,
    pub replacement: Value,
}

pub struct Group {
    pub column: &'static str,
    pub updates: Vec<Update>,
}

pub struct Case {
    pub profile: &'static str,
    pub header: u8,
    pub recipe: &'static str,
    pub definition: u32,
    pub data: u32,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub expected: Vec<Vec<Value>>,
    pub groups: Vec<Group>,
    pub changed: usize,
}

pub fn binary(length: usize, salt: usize) -> Value {
    Value::Binary((0..length).map(|index| (index * 73 + salt) as u8).collect())
}

pub fn original_rows() -> Vec<Vec<Value>> {
    vec![
        vec![Value::Int32(1), Value::Null, Value::Null, Value::Null],
        vec![
            Value::Int32(2),
            Value::Int32(7),
            Value::Binary(Vec::new()),
            Value::Text(String::new()),
        ],
        vec![
            Value::Int32(3),
            Value::Int32(7),
            binary(65, 3),
            Value::Text("Ā".repeat(33)),
        ],
        vec![
            Value::Int32(4),
            Value::Int32(8),
            binary(4076, 4),
            Value::Text("Ā".repeat(2038)),
        ],
        vec![
            Value::Int32(5),
            Value::Null,
            binary(4077, 5),
            Value::Text("Ā".repeat(2039)),
        ],
        vec![
            Value::Int32(6),
            Value::Int32(9),
            binary(65, 6),
            Value::Text("ABC".into()),
        ],
    ]
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "1d1b8bfc50336ea16692004b3b3ec23a356260e003fc7e23eb87867d9a6be782",
    );
    let mut images = Vec::new();
    for &(profile, recipe, filename, length, hash) in PINS {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        verify(&raw, hash);
        images.push((profile, recipe, raw));
    }
    assert_eq!(images.len(), 32);
    let mut result = Vec::new();
    for &(profile, header, definition, data) in PROFILES {
        let before = &images
            .iter()
            .find(|(p, recipe, _)| *p == profile && *recipe == "original")
            .unwrap()
            .2;
        for &recipe in RECIPES {
            let mut groups = Vec::new();
            let mut expected = original_rows();
            let (row, blob, memo) = match recipe {
                "joint-single65-66" => (2, Some(binary(65, 13)), Some(Value::Text("β".repeat(33)))),
                "joint-single4076" => (
                    3,
                    Some(binary(4076, 14)),
                    Some(Value::Text("β".repeat(2038))),
                ),
                "joint-chain4077-4078" => (
                    4,
                    Some(binary(4077, 15)),
                    Some(Value::Text("β".repeat(2039))),
                ),
                "memo-last-tail-shrink" => (4, None, Some(Value::Text("β".repeat(2038)))),
                "blob-last-slot-shrink" => (5, Some(binary(63, 16)), None),
                "noop-chain" => (4, None, Some(Value::Text("Ā".repeat(2039)))),
                "empty-group" => (0, None, None),
                _ => unreachable!(),
            };
            for (column, position, replacement) in [("Blob", 2, blob), ("Memo", 3, memo)] {
                if let Some(replacement) = replacement {
                    groups.push(Group {
                        column,
                        updates: vec![Update {
                            row,
                            expected: expected[row][position].clone(),
                            replacement: replacement.clone(),
                        }],
                    });
                    expected[row][position] = replacement;
                }
            }
            if recipe == "empty-group" {
                groups.push(Group {
                    column: "Memo",
                    updates: Vec::new(),
                });
            }
            let after = &images
                .iter()
                .find(|(p, name, _)| *p == profile && *name == recipe)
                .unwrap()
                .2;
            result.push(Case {
                profile,
                header,
                recipe,
                definition,
                data,
                before: before.clone(),
                after: after.clone(),
                expected,
                groups,
                changed: usize::from(!matches!(recipe, "noop-chain" | "empty-group")),
            });
        }
    }
    assert_eq!(result.len(), 28);
    result
}
