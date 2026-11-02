//! Continuation literals share the existing independent original value contracts.
use super::genuine_external_lval_update::{Case, Group, Update, binary, original_rows};
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
        "grow-shared-last-blob",
        "V2016-raw-grow-shared-last-blob.accdb",
        446464,
        "97708d25b2b6fb7608c3a66109daf34efb6b6338a6da15df257b65118930908b",
    ),
    (
        "V2016-raw",
        "grow-chain-last-memo",
        "V2016-raw-grow-chain-last-memo.accdb",
        446464,
        "4fb824f12eabfd7e3690309cdab974b89f11024eca35061f16e10ee839c076a6",
    ),
    (
        "V2016-raw",
        "joint-terminal-growth",
        "V2016-raw-joint-terminal-growth.accdb",
        446464,
        "1b05804020188b3602414c34b45fceb0109df5c622608f1239e5910a3c960225",
    ),
    (
        "V2016-raw",
        "clear-single-blob",
        "V2016-raw-clear-single-blob.accdb",
        446464,
        "b52a5baba9e720fb9ebe9457c1ebe28d88444c1329419cdd4ea49f33edbe7194",
    ),
    (
        "V2016-raw",
        "clear-single-memo",
        "V2016-raw-clear-single-memo.accdb",
        446464,
        "8b90f2ef1030abecac83b0b9f34ee3e71f19903e5f27ad9cfda235983ce92949",
    ),
    (
        "V2016-raw",
        "clear-single-both",
        "V2016-raw-clear-single-both.accdb",
        446464,
        "433ad31dfe6aa92954c4d3f42c61856aafb166bd95eb2052fbdd54c5e3735fe7",
    ),
    (
        "V2016-raw",
        "empty-single-blob",
        "V2016-raw-empty-single-blob.accdb",
        446464,
        "b9fe7f2607fdf2a212e05dccd315439a5137e47036922497bd065162e2aef725",
    ),
    (
        "V2016-raw",
        "clear-memo-chain",
        "V2016-raw-clear-memo-chain.accdb",
        446464,
        "ab917b5fe086c075a453d81f45a0faaa26bedcdb50602e854d9664eee6a56f06",
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
        "grow-shared-last-blob",
        "V2016-compressed-grow-shared-last-blob.accdb",
        446464,
        "9ab9230e94b1fadb75d2f9e968ae2f1584f72aa1eb4d6678887080767da499d3",
    ),
    (
        "V2016-compressed",
        "grow-chain-last-memo",
        "V2016-compressed-grow-chain-last-memo.accdb",
        446464,
        "3eaddfce3f22a235c4848c8f9df38027a4d9ef0be7ae819dbeccf46a4393bf7d",
    ),
    (
        "V2016-compressed",
        "joint-terminal-growth",
        "V2016-compressed-joint-terminal-growth.accdb",
        446464,
        "04189ac41a8f7a2b65fbd3ea5408da3a2d18f7c4679f6c62d16ce1ec622c90dc",
    ),
    (
        "V2016-compressed",
        "clear-single-blob",
        "V2016-compressed-clear-single-blob.accdb",
        446464,
        "d0f43df3e3452306e5f6344ba1626028052049a9374a6c2fa6b0c7a5f15bedc1",
    ),
    (
        "V2016-compressed",
        "clear-single-memo",
        "V2016-compressed-clear-single-memo.accdb",
        446464,
        "78713919d84e79007a3c783337bceca40a7b202d85eedeebfe62596c3cb4b108",
    ),
    (
        "V2016-compressed",
        "clear-single-both",
        "V2016-compressed-clear-single-both.accdb",
        446464,
        "5406e44ba0e0e0d394a0715657c87de02cbeed58f6b754be4b6e7f6a919be3de",
    ),
    (
        "V2016-compressed",
        "empty-single-blob",
        "V2016-compressed-empty-single-blob.accdb",
        446464,
        "84481dc137f8d456270b28832613293efce754ce1485ee859aab5fd06eb39699",
    ),
    (
        "V2016-compressed",
        "clear-memo-chain",
        "V2016-compressed-clear-memo-chain.accdb",
        446464,
        "b8b2996732192de2a5a81e662608ec802322ed6f24e4121c193239015bf72111",
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
        "grow-shared-last-blob",
        "V2019-raw-grow-shared-last-blob.accdb",
        520192,
        "6c71a41be056039124067da636ddabeeab1d53abff8463828693c86434c0e7a8",
    ),
    (
        "V2019-raw",
        "grow-chain-last-memo",
        "V2019-raw-grow-chain-last-memo.accdb",
        520192,
        "49098edbc812c1fd0269837727d40500671104298eaa0e8fa1a5ee34172f108d",
    ),
    (
        "V2019-raw",
        "joint-terminal-growth",
        "V2019-raw-joint-terminal-growth.accdb",
        520192,
        "e34e2bb5252633406d19f064de1b5f56170c6ed835ff9bef29312ffd4abb9f5f",
    ),
    (
        "V2019-raw",
        "clear-single-blob",
        "V2019-raw-clear-single-blob.accdb",
        520192,
        "7b07a3402e5381167c722edec4b3a3a8f16122d244a56540b12302e355be2176",
    ),
    (
        "V2019-raw",
        "clear-single-memo",
        "V2019-raw-clear-single-memo.accdb",
        520192,
        "16fccd265c03d3ec9c59e24ea7fd613b1c3645a7142d16704e0ad83b24def0f8",
    ),
    (
        "V2019-raw",
        "clear-single-both",
        "V2019-raw-clear-single-both.accdb",
        520192,
        "a37330fd331721a2e9729bfa430ecf609785267f37d8530486abce0aa1136537",
    ),
    (
        "V2019-raw",
        "empty-single-blob",
        "V2019-raw-empty-single-blob.accdb",
        520192,
        "75ea6a96256a00a872214cb5752385e6c6f51221a04811b472d6cfe47c70de56",
    ),
    (
        "V2019-raw",
        "clear-memo-chain",
        "V2019-raw-clear-memo-chain.accdb",
        520192,
        "5012899696f38d9468ccf9fa7e24fa65357e18431b6b007eb1df7f793e66a891",
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
        "grow-shared-last-blob",
        "V2019-compressed-grow-shared-last-blob.accdb",
        520192,
        "ee56391acc4d03256475d92092a08b5a2df741f8a0a2a233452ff95caa9cfb2a",
    ),
    (
        "V2019-compressed",
        "grow-chain-last-memo",
        "V2019-compressed-grow-chain-last-memo.accdb",
        520192,
        "ea686b3957a4a62dc365a949f786c9b8f915de62dd8ee2664d0e005c466d99b4",
    ),
    (
        "V2019-compressed",
        "joint-terminal-growth",
        "V2019-compressed-joint-terminal-growth.accdb",
        520192,
        "b4bafe36aaa658ed5a0304d0c36b4947a169b8ae80cc1ad46f4954a4e9a9c2af",
    ),
    (
        "V2019-compressed",
        "clear-single-blob",
        "V2019-compressed-clear-single-blob.accdb",
        520192,
        "ab962d6210d77f97dd0c0598c768bee22095ca6ef9cee11f1f5f6b1b2b456122",
    ),
    (
        "V2019-compressed",
        "clear-single-memo",
        "V2019-compressed-clear-single-memo.accdb",
        520192,
        "a09ccd42ff634bf8880f9bb4bfccb2b2ce424481f74cdcb58c424c268eb2e153",
    ),
    (
        "V2019-compressed",
        "clear-single-both",
        "V2019-compressed-clear-single-both.accdb",
        520192,
        "9b815fab3d997bf7e826bd6d25dce748c72fc67a3548cad5c3df79084568e477",
    ),
    (
        "V2019-compressed",
        "empty-single-blob",
        "V2019-compressed-empty-single-blob.accdb",
        520192,
        "ca38b2f33a932ad27653db79cd37f321d27f2f9331fae6bb214586b08aa3ae8c",
    ),
    (
        "V2019-compressed",
        "clear-memo-chain",
        "V2019-compressed-clear-memo-chain.accdb",
        520192,
        "6c19dfda4ff3c9f73903fc44d454f5f3de86a58b77c08833226dc5a36729cf4c",
    ),
];

const PROFILES: &[(&str, u8, u32, u32)] = &[
    ("V2016-raw", 5, 89, 93),
    ("V2016-compressed", 5, 89, 93),
    ("V2019-raw", 6, 106, 110),
    ("V2019-compressed", 6, 106, 110),
];

const RECIPES: &[&str] = &[
    "grow-shared-last-blob",
    "grow-chain-last-memo",
    "joint-terminal-growth",
    "clear-single-blob",
    "clear-single-memo",
    "clear-single-both",
    "empty-single-blob",
    "clear-memo-chain",
];

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_CONTINUATION_SOURCE_DIR").unwrap(),
    );
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        "b8468b51e6d6166ea31d1700b0b2c2a3b0722cbc53494c02fb660be4aa648424",
    );
    let mut images = Vec::new();
    for &(profile, recipe, filename, length, hash) in PINS {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), length);
        verify(&raw, hash);
        images.push((profile, recipe, raw));
    }
    assert_eq!(images.len(), 36);
    let mut cases = Vec::new();
    for &(profile, header, definition, data) in PROFILES {
        let before = &images
            .iter()
            .find(|(p, recipe, _)| *p == profile && *recipe == "original")
            .unwrap()
            .2;
        for &recipe in RECIPES {
            let mut expected = original_rows();
            let requests = match recipe {
                "grow-shared-last-blob" => vec![("Blob", 5, 2, binary(66, 26))],
                "grow-chain-last-memo" => vec![("Memo", 4, 3, Value::Text("β".repeat(2040)))],
                "joint-terminal-growth" => vec![
                    ("Memo", 4, 3, Value::Text("β".repeat(2040))),
                    ("Blob", 5, 2, binary(66, 26)),
                ],
                "clear-single-blob" => vec![("Blob", 2, 2, Value::Null)],
                "clear-single-memo" => vec![("Memo", 2, 3, Value::Null)],
                "clear-single-both" => {
                    vec![("Blob", 2, 2, Value::Null), ("Memo", 2, 3, Value::Null)]
                }
                "empty-single-blob" => vec![("Blob", 2, 2, Value::Binary(Vec::new()))],
                "clear-memo-chain" => vec![("Memo", 4, 3, Value::Null)],
                _ => unreachable!(),
            };
            let mut groups = Vec::new();
            for (column, row, position, replacement) in requests {
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
            let after = &images
                .iter()
                .find(|(p, name, _)| *p == profile && *name == recipe)
                .unwrap()
                .2;
            cases.push(Case {
                profile,
                header,
                recipe,
                definition,
                data,
                before: before.clone(),
                after: after.clone(),
                expected,
                groups,
                changed: if recipe == "joint-terminal-growth" {
                    2
                } else {
                    1
                },
            });
        }
    }
    assert_eq!(cases.len(), 32);
    cases
}

fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, payload) = match value {
        Value::Null => (0, Vec::new()),
        Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
        Value::Text(value) => (6, value.as_bytes().to_vec()),
        Value::Binary(value) => (7, value.clone()),
        _ => panic!("only original literal source carriers enter this wire"),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(&payload);
}

pub fn intent_wire(case: &Case, before_digest: [u8; 32]) -> Vec<u8> {
    let mut body = Vec::from(*b"Items");
    let mut assignments = 0_u32;
    for group in &case.groups {
        let mut cells = Vec::new();
        for update in &group.updates {
            let mut cell = (update.row as u64).to_le_bytes().to_vec();
            carrier(&mut cell, &update.expected);
            carrier(&mut cell, &update.replacement);
            cells.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
            cells.extend_from_slice(&cell);
            assignments += 1;
        }
        body.extend_from_slice(&((20 + group.column.len() + cells.len()) as u32).to_le_bytes());
        body.extend_from_slice(&(group.column.len() as u32).to_le_bytes());
        body.extend_from_slice(&(group.updates.len() as u32).to_le_bytes());
        body.extend_from_slice(&[1, 0, 0, 0]);
        body.extend_from_slice(&0_u32.to_le_bytes());
        body.extend_from_slice(group.column.as_bytes());
        body.extend_from_slice(&cells);
    }
    let mut raw = Vec::from(*b"TCNI0005");
    raw.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    raw.extend_from_slice(&[5, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    raw.extend_from_slice(&before_digest);
    for number in [5, case.groups.len() as u32, assignments, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(&body);
    raw
}

pub fn retired_pages(case: &Case) -> Vec<u32> {
    let delta = u32::from(case.header == 6) * 17;
    let pages = match case.recipe {
        "clear-single-blob" | "empty-single-blob" => vec![95],
        "clear-single-memo" => vec![96],
        "clear-single-both" => vec![95, 96],
        "clear-memo-chain" => vec![101, 102],
        _ => unreachable!(),
    };
    pages.into_iter().map(|page| page + delta).collect()
}

/// SQL spellings exist only for the independently qualified NULL replacements.
pub fn retirement_sql(case: &Case) -> Option<&'static str> {
    match case.recipe {
        "clear-single-blob" => Some("UPDATE Items SET [Blob]=NULL WHERE [ID]=3"),
        "clear-single-memo" => Some("UPDATE Items SET [Memo]=NULL WHERE [ID]=3"),
        "clear-single-both" => Some("UPDATE Items SET [Blob]=NULL,[Memo]=NULL WHERE [ID]=3"),
        "clear-memo-chain" => Some("UPDATE Items SET [Memo]=NULL WHERE [ID]=5"),
        _ => None,
    }
}
