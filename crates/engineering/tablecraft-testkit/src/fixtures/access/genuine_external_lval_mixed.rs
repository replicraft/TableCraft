//! Independent literal mixed-retirement views; no engine codec or planner imports.
use super::genuine_external_lval_update::{binary, original_rows};
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const MANIFEST_SHA256: &str = "65caec78bc86af0ea48b8dbed008d73b5a1c6998aef3da197cbbb1bae288e560";
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
        "clear-blob3-grow-memo5",
        "V2016-raw-clear-blob3-grow-memo5.accdb",
        446464,
        "7a16d2e6786a1c4f58cf0d69bfd3869d43a869f37da49c6ad86d293f7589f9e6",
    ),
    (
        "V2016-raw",
        "clear-memo3-reuse-blob3",
        "V2016-raw-clear-memo3-reuse-blob3.accdb",
        446464,
        "bbba0c2d02c30092da60f4606c3af5e82d8c6aff6b74e47f895e1c1c6507cd31",
    ),
    (
        "V2016-raw",
        "clear-blob3-grow-memo3",
        "V2016-raw-clear-blob3-grow-memo3.accdb",
        446464,
        "c2a2196af8bd96d04d8077fae2c654abcaa928f97410ffd765d0a3497707c36e",
    ),
    (
        "V2016-raw",
        "clear-memo3-shrink-blob3",
        "V2016-raw-clear-memo3-shrink-blob3.accdb",
        446464,
        "a94e61d0df0767b6527b4a78e055427edacf41e3461d3df88dc9d94110cf6300",
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
        "clear-blob3-grow-memo5",
        "V2016-compressed-clear-blob3-grow-memo5.accdb",
        446464,
        "6583f062468b8dbf1a33682ac5944fb60f08cb8f265a58646e568cc76191a43c",
    ),
    (
        "V2016-compressed",
        "clear-memo3-reuse-blob3",
        "V2016-compressed-clear-memo3-reuse-blob3.accdb",
        446464,
        "dbd61075628113fec73b7bfa6f89224c49c378225f139c3b03e6762a6f617114",
    ),
    (
        "V2016-compressed",
        "clear-blob3-grow-memo3",
        "V2016-compressed-clear-blob3-grow-memo3.accdb",
        446464,
        "5174fe64991e52a95456be482d99e4736a0e4b2258ae7bf9fade878c26ad7bfc",
    ),
    (
        "V2016-compressed",
        "clear-memo3-shrink-blob3",
        "V2016-compressed-clear-memo3-shrink-blob3.accdb",
        446464,
        "69fd558a941bdc35954520d3ed6f90e947a03e95b2d1902d21dd5a7e25ff13f7",
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
        "clear-blob3-grow-memo5",
        "V2019-raw-clear-blob3-grow-memo5.accdb",
        520192,
        "1f95966ba03a3f180ce169508bdab174bb575cb359e365d6ea111eaa5306944e",
    ),
    (
        "V2019-raw",
        "clear-memo3-reuse-blob3",
        "V2019-raw-clear-memo3-reuse-blob3.accdb",
        520192,
        "bb4a77747cde3120e1dd3fb94fc8d6d6ed0dacb68702b40d27b339714eda6034",
    ),
    (
        "V2019-raw",
        "clear-blob3-grow-memo3",
        "V2019-raw-clear-blob3-grow-memo3.accdb",
        520192,
        "c2e8b6a663f962a9183c5057fc3dfe2aaac22d62dfdfd4f7d1523bc4911c0527",
    ),
    (
        "V2019-raw",
        "clear-memo3-shrink-blob3",
        "V2019-raw-clear-memo3-shrink-blob3.accdb",
        520192,
        "d053bbd400aae7387c7d5dd11c0091fc80006699528845324bad15153204cf7f",
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
        "clear-blob3-grow-memo5",
        "V2019-compressed-clear-blob3-grow-memo5.accdb",
        520192,
        "495878046fa6f424fe4ec3c14184966e8768614d4ce937aaa9f87ccbc8a788b7",
    ),
    (
        "V2019-compressed",
        "clear-memo3-reuse-blob3",
        "V2019-compressed-clear-memo3-reuse-blob3.accdb",
        520192,
        "3d16c72df3996c86a8dfe596ea809e6181a175d712d909bdea3e823166824fd7",
    ),
    (
        "V2019-compressed",
        "clear-blob3-grow-memo3",
        "V2019-compressed-clear-blob3-grow-memo3.accdb",
        520192,
        "e191a502ba135ab096d9ee2b09a21e5c9f12ca111c997a47e7ca7aca653e9449",
    ),
    (
        "V2019-compressed",
        "clear-memo3-shrink-blob3",
        "V2019-compressed-clear-memo3-shrink-blob3.accdb",
        520192,
        "288db37bdc710b8caecb4f70625a412efc5ebf15ba1d5de8dcac711f308cca13",
    ),
];
const PROFILES: &[(&str, u8, usize)] = &[
    ("V2016-raw", 5, 89),
    ("V2016-compressed", 5, 89),
    ("V2019-raw", 6, 106),
    ("V2019-compressed", 6, 106),
];
const RECIPES: &[&str] = &[
    "original",
    "clear-blob3-grow-memo5",
    "clear-memo3-reuse-blob3",
    "clear-blob3-grow-memo3",
    "clear-memo3-shrink-blob3",
];

pub struct Case {
    pub profile: &'static str,
    pub header: u8,
    pub definition: usize,
    pub data: usize,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
}

fn word(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

pub fn rows(recipe: &str) -> Vec<Vec<Value>> {
    let mut rows = original_rows();
    match recipe {
        "original" => {}
        "clear-blob3-grow-memo5" => {
            rows[2][2] = Value::Null;
            rows[4][3] = Value::Text("β".repeat(2040));
        }
        "clear-memo3-reuse-blob3" => {
            rows[2][3] = Value::Null;
            rows[2][2] = binary(65, 13);
        }
        "clear-blob3-grow-memo3" => {
            rows[2][2] = Value::Null;
            rows[2][3] = Value::Text("β".repeat(34));
        }
        "clear-memo3-shrink-blob3" => {
            rows[2][3] = Value::Null;
            rows[2][2] = binary(63, 16);
        }
        _ => panic!("only independently frozen mixed recipes"),
    }
    rows
}

fn parent(
    before: &[u8],
    after: &mut [u8],
    data: usize,
    identity: usize,
    lengths: [Option<u32>; 2],
) {
    let slot = identity - 1;
    let base = data * 4096;
    let start = usize::from(word(before, base + 14 + 2 * slot) & 0x1fff);
    let end = if slot == 0 {
        4096
    } else {
        usize::from(word(before, base + 14 + 2 * (slot - 1)) & 0x1fff)
    };
    assert_eq!(end - start, 43);
    let original = &before[base + start..base + end];
    assert_eq!(word(original, 0), 4);
    assert_eq!(word(original, 34), 34);
    assert_eq!(word(original, 36), 22);
    assert_eq!(word(original, 38), 10);
    assert_eq!(word(original, 40), 2);
    let mut row = original.to_vec();
    let mut active = Vec::new();
    let mut mask = original[42];
    let mut offsets = [10_u16; 2];
    for (column, length) in lengths.into_iter().enumerate() {
        offsets[column] = 10 + active.len() as u16;
        if let Some(length) = length {
            let from = 10 + 12 * column;
            let mut descriptor = original[from..from + 12].to_vec();
            let old = u32::from_le_bytes(descriptor[..4].try_into().unwrap());
            assert!(matches!(old & 0xc0000000, 0 | 0x40000000));
            descriptor[..4].copy_from_slice(&((old & 0xc0000000) | length).to_le_bytes());
            active.extend_from_slice(&descriptor);
            mask |= 1 << (column + 2);
        } else {
            mask &= !(1 << (column + 2));
        }
    }
    let eod = 10 + active.len();
    assert!(eod <= 34);
    row[10..eod].copy_from_slice(&active);
    row[34..36].copy_from_slice(&(eod as u16).to_le_bytes());
    row[36..38].copy_from_slice(&offsets[1].to_le_bytes());
    row[38..40].copy_from_slice(&offsets[0].to_le_bytes());
    row[42] = mask;
    assert_eq!(&row[eod..34], &original[eod..34]);
    after[base + start..base + end].copy_from_slice(&row);
}

fn retire(before: &[u8], after: &mut [u8], definition: usize, column: usize, page: usize) {
    let map_page = (definition + 1) * 4096;
    let (owned, hint) = match column {
        2 => (3751, 3682),
        3 => (3613, 3544),
        _ => unreachable!(),
    };
    let bit = 1 << (page % 8);
    let owned = map_page + owned + 5 + page / 8;
    let hint = map_page + hint + 5 + page / 8;
    let global = 4096 + 4027 + 5 + page / 8;
    assert_ne!(before[owned] & bit, 0);
    assert_eq!(before[hint] & bit, 0);
    assert_eq!(before[global] & bit, 0);
    assert_eq!(&before[page * 4096..page * 4096 + 2], &[1, 1]);
    assert_eq!(&before[page * 4096 + 4..page * 4096 + 8], b"LVAL");
    assert_eq!(word(before, page * 4096 + 12), 1);
    after[owned] &= !bit;
    after[hint] &= !bit;
    after[global] |= bit;
    after[page * 4096..page * 4096 + 4].fill(0);
    assert_eq!(
        &after[page * 4096 + 4..(page + 1) * 4096],
        &before[page * 4096 + 4..(page + 1) * 4096]
    );
}

fn single(before: &[u8], after: &mut [u8], page: usize, new_start: usize, bytes: &[u8]) {
    let base = page * 4096;
    let old_start = usize::from(word(before, base + 14));
    let free =
        isize::try_from(word(before, base + 2)).unwrap() + new_start as isize - old_start as isize;
    assert_eq!(word(before, base + 12), 1);
    assert_eq!(4096 - new_start, bytes.len());
    after[base + new_start..base + 4096].copy_from_slice(bytes);
    after[base + 14..base + 16].copy_from_slice(&(new_start as u16).to_le_bytes());
    after[base + 2..base + 4].copy_from_slice(&u16::try_from(free).unwrap().to_le_bytes());
    if new_start > old_start {
        assert_eq!(
            &after[base + old_start..base + new_start],
            &before[base + old_start..base + new_start]
        );
    }
}

pub fn expected(before: &[u8], header: u8, recipe: &str) -> Vec<u8> {
    let mut after = before.to_vec();
    let shift = if header == 5 { 0 } else { 17 };
    let definition = 89 + shift;
    let data = 93 + shift;
    match recipe {
        "original" => {}
        "clear-blob3-grow-memo5" => {
            parent(before, &mut after, data, 3, [None, Some(66)]);
            parent(before, &mut after, data, 5, [Some(4077), Some(4080)]);
            retire(before, &mut after, definition, 2, 95 + shift);
            let payload = "β"
                .repeat(2040)
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            let first = (101 + shift) * 4096;
            after[first + 24..first + 4096].copy_from_slice(&payload[..4072]);
            let last = (102 + shift) * 4096;
            assert_eq!(word(before, last + 14), 4086);
            assert_eq!(word(before, last + 2), 4070);
            after[last + 2..last + 4].copy_from_slice(&4068_u16.to_le_bytes());
            after[last + 14..last + 16].copy_from_slice(&4084_u16.to_le_bytes());
            after[last + 4084..last + 4088].fill(0);
            after[last + 4088..last + 4096].copy_from_slice(&payload[4072..]);
        }
        "clear-memo3-reuse-blob3" => {
            parent(before, &mut after, data, 3, [Some(65), None]);
            retire(before, &mut after, definition, 3, 96 + shift);
            let Value::Binary(payload) = binary(65, 13) else {
                unreachable!()
            };
            single(before, &mut after, 95 + shift, 4031, &payload);
        }
        "clear-blob3-grow-memo3" => {
            parent(before, &mut after, data, 3, [None, Some(68)]);
            retire(before, &mut after, definition, 2, 95 + shift);
            let payload = "β"
                .repeat(34)
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            single(before, &mut after, 96 + shift, 4028, &payload);
        }
        "clear-memo3-shrink-blob3" => {
            parent(before, &mut after, data, 3, [Some(63), None]);
            retire(before, &mut after, definition, 3, 96 + shift);
            let Value::Binary(payload) = binary(63, 16) else {
                unreachable!()
            };
            single(before, &mut after, 95 + shift, 4033, &payload);
        }
        _ => panic!("only frozen mixed recipes"),
    }
    for page in [103 + shift, 104 + shift] {
        assert_eq!(
            &after[page * 4096..(page + 1) * 4096],
            &before[page * 4096..(page + 1) * 4096]
        );
    }
    after
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_EXTERNAL_LVAL_MIXED_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    verify(
        &fs::read(directory.join("manifest.json")).unwrap(),
        MANIFEST_SHA256,
    );
    let mut images = Vec::new();
    for &(profile, recipe, filename, length, sha) in PINS {
        let bytes = fs::read(directory.join(filename)).unwrap();
        assert_eq!(bytes.len(), length);
        verify(&bytes, sha);
        images.push((profile, recipe, bytes));
    }
    assert_eq!(images.len(), 20);
    let mut cases = Vec::new();
    for &(profile, header, definition) in PROFILES {
        let before = &images
            .iter()
            .find(|(p, r, _)| *p == profile && *r == "original")
            .unwrap()
            .2;
        for &recipe in RECIPES {
            let after = &images
                .iter()
                .find(|(p, r, _)| *p == profile && *r == recipe)
                .unwrap()
                .2;
            cases.push(Case {
                profile,
                header,
                definition,
                data: definition + 4,
                recipe,
                before: before.clone(),
                after: after.clone(),
                rows: rows(recipe),
            });
        }
    }
    assert_eq!(cases.len(), 20);
    cases
}
