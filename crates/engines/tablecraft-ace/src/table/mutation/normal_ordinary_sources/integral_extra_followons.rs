//! Read-only exact literal qualification of fresh numeric follow-on source images.
use crate::{
    EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(198);
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
type Row = (usize, i64, Option<i64>, i32);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn u16_at(raw: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(raw[at..at + 2].try_into().unwrap())
}

fn rows(table: &str, recipe: &str) -> Vec<Row> {
    let wide = table.ends_with("WidePrimary");
    let null = table.ends_with("AllNull");
    let mut result = vec![
        (
            0,
            if wide { i64::MIN } else { 1 },
            if null { None } else { Some(i64::MIN) },
            17,
        ),
        (1, if wide { 0 } else { 2 }, None, 34),
        (
            2,
            if wide { i64::MAX } else { 3 },
            if null { None } else { Some(i64::MAX) },
            51,
        ),
    ];
    match recipe {
        "before" => {}
        "insert-null" => result.push((3, if wide { 1 } else { 4 }, None, 68)),
        "insert-zero" => result.push((3, if wide { 1 } else { 4 }, Some(0), 68)),
        "delete-all" => result.clear(),
        _ => unreachable!(),
    }
    result
}

fn scalar(table: &str, value: i64) -> Value {
    if table.starts_with("Currency") {
        Value::Currency(Currency::from_scaled(value))
    } else {
        Value::Int64(value)
    }
}

fn values(table: &str, row: Row) -> Vec<Value> {
    vec![
        if table.ends_with("WidePrimary") {
            scalar(table, row.1)
        } else {
            Value::Int32(i32::try_from(row.1).unwrap())
        },
        row.2.map_or(Value::Null, |value| scalar(table, value)),
        Value::Int32(row.3),
    ]
}

fn order(rows: &mut [Row], physical: usize) {
    rows.sort_by(|left, right| {
        let left_key = if physical == 0 { Some(left.1) } else { left.2 };
        let right_key = if physical == 0 {
            Some(right.1)
        } else {
            right.2
        };
        let compared = left_key.cmp(&right_key);
        let compared = if physical == 2 {
            compared.reverse()
        } else {
            compared
        };
        if compared == Ordering::Equal {
            left.0.cmp(&right.0)
        } else {
            compared
        }
    });
}

fn history(table: &str, recipe: &str, physical: usize) -> u32 {
    let initial = if physical != 0 && table.ends_with("AllNull") {
        1
    } else {
        3
    };
    initial + u32::from(recipe.starts_with("insert-") && (physical == 0 || recipe == "insert-zero"))
}

fn literal(
    original: &[u8],
    table: &str,
    definition: u32,
    data: u32,
    roots: [u32; 3],
    recipe: &str,
) -> Vec<u8> {
    let mut after = original.to_vec();
    if recipe == "before" {
        return after;
    }
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    let wide = table.ends_with("WidePrimary");
    let length = if wide { 23 } else { 19 };
    let final_rows = rows(table, recipe);
    if recipe.starts_with("insert-") {
        let mut record = 3_u16.to_le_bytes().to_vec();
        if wide {
            record.extend_from_slice(&1_i64.to_le_bytes());
        } else {
            record.extend_from_slice(&4_i32.to_le_bytes());
        }
        record.extend_from_slice(&0_i64.to_le_bytes());
        record.extend_from_slice(&68_i32.to_le_bytes());
        record.push(if recipe == "insert-null" { 5 } else { 7 });
        assert_eq!(record.len(), length);
        let start = 4096 - 4 * length;
        assert_eq!(&original[page + 20..page + 22], &[0, 0]);
        after[page + start..page + start + length].copy_from_slice(&record);
        after[page + 20..page + 22].copy_from_slice(&(start as u16).to_le_bytes());
        after[page + 12..page + 14].copy_from_slice(&4_u16.to_le_bytes());
        after[page + 2..page + 4].copy_from_slice(&((start - 22) as u16).to_le_bytes());
    } else {
        assert_eq!(recipe, "delete-all");
        for slot in 0..3 {
            let at = page + 14 + slot * 2;
            after[at..at + 2].copy_from_slice(&(u16_at(original, at) | 0xc000).to_le_bytes());
        }
    }
    after[td + 16..td + 20].copy_from_slice(&(final_rows.len() as u32).to_le_bytes());
    for (physical, root) in roots.iter().enumerate() {
        let mut sorted = final_rows.clone();
        order(&mut sorted, physical);
        let mut bytes = Vec::new();
        let mut bitmap = [0_u8; 453];
        for row in sorted {
            let value = if physical == 0 { Some(row.1) } else { row.2 };
            let width = if physical == 0 && !wide { 4 } else { 8 };
            if let Some(value) = value {
                bytes.push(if physical == 2 { 128 } else { 127 });
                let mask = if width == 4 {
                    u32::MAX as u64
                } else {
                    u64::MAX
                };
                let ordered = (value as u64 & mask) ^ (1_u64 << (8 * width - 1));
                let ordered = if physical == 2 {
                    ordered ^ mask
                } else {
                    ordered
                };
                bytes.extend_from_slice(&ordered.to_be_bytes()[8 - width..]);
            } else {
                bytes.push(if physical == 2 { 255 } else { 0 });
            }
            bytes.extend_from_slice(&((data << 8) | row.0 as u32).to_be_bytes());
            let end = bytes.len();
            bitmap[end / 8] |= 1 << (end % 8);
        }
        let at = *root as usize * 4096;
        after[at + 2..at + 4].copy_from_slice(&((3616 - bytes.len()) as u16).to_le_bytes());
        after[at + 27..at + 480].copy_from_slice(&bitmap);
        after[at + 480..at + 480 + bytes.len()].copy_from_slice(&bytes);
        after[td + 67 + 12 * physical..td + 71 + 12 * physical]
            .copy_from_slice(&history(table, recipe, physical).to_le_bytes());
    }
    after
}

#[test]
#[ignore = "requires frozen fresh numeric originals and48 literal views"]
fn fresh_numeric_followons_have_literal_images_and_complete_native_membership() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR").unwrap());
    let old = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_ORIGINALS_DIR").unwrap());
    assert!(dir.is_absolute() && old.is_absolute());
    assert_eq!(
        digest(&fs::read(dir.join("manifest.json")).unwrap()),
        "70cc4a5675eeddb6559c35a18767cdcccbb4d54401c3c0c525bd7e9fcad74bb4"
    );
    for (header, table, definition, data, roots, recipe, filename, hash) in [
        (
            5_u8,
            "Int64AllNull",
            89_u32,
            94_u32,
            [91, 92, 93],
            "before",
            "h5-Int64AllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "Int64AllNull",
            89_u32,
            94_u32,
            [91, 92, 93],
            "insert-null",
            "h5-Int64AllNull-insert-null.accdb",
            "bdde173f66105ca697130e401fd8a498c3f5a04bf2f367d4661ceb66453cb911",
        ),
        (
            5_u8,
            "Int64AllNull",
            89_u32,
            94_u32,
            [91, 92, 93],
            "insert-zero",
            "h5-Int64AllNull-insert-zero.accdb",
            "d20a34d871037da0492a86363c9de34225c49508f269a3f0d9a04fc02895e536",
        ),
        (
            5_u8,
            "Int64AllNull",
            89_u32,
            94_u32,
            [91, 92, 93],
            "delete-all",
            "h5-Int64AllNull-delete-all.accdb",
            "6862fecceefbf171d73535b78e88e3ed86692c3f059ce6f7690d1d70deea1c65",
        ),
        (
            5_u8,
            "CurrencyAllNull",
            96_u32,
            101_u32,
            [98, 99, 100],
            "before",
            "h5-CurrencyAllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "CurrencyAllNull",
            96_u32,
            101_u32,
            [98, 99, 100],
            "insert-null",
            "h5-CurrencyAllNull-insert-null.accdb",
            "f0d96ecf58ddf6dcfef9da8ab9040bd68743ab543e45de072c8a6645788f0a64",
        ),
        (
            5_u8,
            "CurrencyAllNull",
            96_u32,
            101_u32,
            [98, 99, 100],
            "insert-zero",
            "h5-CurrencyAllNull-insert-zero.accdb",
            "76c40c8e06d31a63880ee89a0a4a1d1324c51fbd843aed6d316e5aba76eb6bec",
        ),
        (
            5_u8,
            "CurrencyAllNull",
            96_u32,
            101_u32,
            [98, 99, 100],
            "delete-all",
            "h5-CurrencyAllNull-delete-all.accdb",
            "814ecc80c10e29adcfa1a145ef837a75c52c72ee1f2a608c8b54acb3b215df84",
        ),
        (
            5_u8,
            "Int64WidePrimary",
            103_u32,
            108_u32,
            [105, 106, 107],
            "before",
            "h5-Int64WidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "Int64WidePrimary",
            103_u32,
            108_u32,
            [105, 106, 107],
            "insert-null",
            "h5-Int64WidePrimary-insert-null.accdb",
            "1170326fdc67517f60dc35719c4b8d3ee47d34aa00b0e01aa21c8faceebfc0a6",
        ),
        (
            5_u8,
            "Int64WidePrimary",
            103_u32,
            108_u32,
            [105, 106, 107],
            "insert-zero",
            "h5-Int64WidePrimary-insert-zero.accdb",
            "8f67ba4ea2f53174e936ca60c0121ddedff1bccbc2cd83db6d3783e7edc481a0",
        ),
        (
            5_u8,
            "Int64WidePrimary",
            103_u32,
            108_u32,
            [105, 106, 107],
            "delete-all",
            "h5-Int64WidePrimary-delete-all.accdb",
            "ac32cb6bfe4601fe8695584c8363b5ef6b9a30253ae7cd32702cd26397a671b3",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            110_u32,
            115_u32,
            [112, 113, 114],
            "before",
            "h5-CurrencyWidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            110_u32,
            115_u32,
            [112, 113, 114],
            "insert-null",
            "h5-CurrencyWidePrimary-insert-null.accdb",
            "70b6544bdb77bd1f5bfd7808c28f66b6bba2c281a31d73aced09cba54e5dc565",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            110_u32,
            115_u32,
            [112, 113, 114],
            "insert-zero",
            "h5-CurrencyWidePrimary-insert-zero.accdb",
            "5b419c61c89ffd52f9166c39bbc9c58b8cdc3e4915998fae8ce9cafb9bcf77a0",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            110_u32,
            115_u32,
            [112, 113, 114],
            "delete-all",
            "h5-CurrencyWidePrimary-delete-all.accdb",
            "830586432a31c26c3d687d7287750af5a104c5c9d8a99fc415805f7b250c0293",
        ),
        (
            5_u8,
            "Int64StableIds",
            117_u32,
            122_u32,
            [119, 120, 121],
            "before",
            "h5-Int64StableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "Int64StableIds",
            117_u32,
            122_u32,
            [119, 120, 121],
            "insert-null",
            "h5-Int64StableIds-insert-null.accdb",
            "315bd7f996c57cd76656fb3c98b7d6f64b6e6bba27807d21d4bbddd332997bac",
        ),
        (
            5_u8,
            "Int64StableIds",
            117_u32,
            122_u32,
            [119, 120, 121],
            "insert-zero",
            "h5-Int64StableIds-insert-zero.accdb",
            "ece74c91673f2b0534a54d22b8b8583660ce441f86e820a6fb695b9627875c80",
        ),
        (
            5_u8,
            "Int64StableIds",
            117_u32,
            122_u32,
            [119, 120, 121],
            "delete-all",
            "h5-Int64StableIds-delete-all.accdb",
            "156de5a3fef54822c2ccbfc8ffeb58e3100ae9ae2e4bc5b03a9349321327cecb",
        ),
        (
            5_u8,
            "CurrencyStableIds",
            124_u32,
            130_u32,
            [126, 127, 128],
            "before",
            "h5-CurrencyStableIds-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
        ),
        (
            5_u8,
            "CurrencyStableIds",
            124_u32,
            130_u32,
            [126, 127, 128],
            "insert-null",
            "h5-CurrencyStableIds-insert-null.accdb",
            "4881c8aadf202e3f8696934e046abb27f4b5e0206172f7c1773d9fa1ca8d57e1",
        ),
        (
            5_u8,
            "CurrencyStableIds",
            124_u32,
            130_u32,
            [126, 127, 128],
            "insert-zero",
            "h5-CurrencyStableIds-insert-zero.accdb",
            "01b0d06bc96dd42f4e4d3c90ec4a1bd4ede79255ca675ecd3fffa0737fcca1c6",
        ),
        (
            5_u8,
            "CurrencyStableIds",
            124_u32,
            130_u32,
            [126, 127, 128],
            "delete-all",
            "h5-CurrencyStableIds-delete-all.accdb",
            "56dd7f21e26cee5b577af95c8f8bd65877aaa65dfe8f601c205c2462522891b1",
        ),
        (
            6_u8,
            "Int64AllNull",
            106_u32,
            111_u32,
            [108, 109, 110],
            "before",
            "h6-Int64AllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "Int64AllNull",
            106_u32,
            111_u32,
            [108, 109, 110],
            "insert-null",
            "h6-Int64AllNull-insert-null.accdb",
            "e615b7cee04b0ebcdc39a12cd87e5bd97f84772caccd1197cfaf0887df44dfd1",
        ),
        (
            6_u8,
            "Int64AllNull",
            106_u32,
            111_u32,
            [108, 109, 110],
            "insert-zero",
            "h6-Int64AllNull-insert-zero.accdb",
            "00708ce0875a21490d16d46b98b4c1fbc994e0f612c009147c65eafc958ce0c1",
        ),
        (
            6_u8,
            "Int64AllNull",
            106_u32,
            111_u32,
            [108, 109, 110],
            "delete-all",
            "h6-Int64AllNull-delete-all.accdb",
            "7ddbf86b818a8af4eb24b103907f262ace97f86accd5d0fb73b288a6d6c902de",
        ),
        (
            6_u8,
            "CurrencyAllNull",
            113_u32,
            118_u32,
            [115, 116, 117],
            "before",
            "h6-CurrencyAllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "CurrencyAllNull",
            113_u32,
            118_u32,
            [115, 116, 117],
            "insert-null",
            "h6-CurrencyAllNull-insert-null.accdb",
            "405c2da1bbe6443c67ed4dc4c3a9076458522400cf4c7eb8c26fa5c31865cabb",
        ),
        (
            6_u8,
            "CurrencyAllNull",
            113_u32,
            118_u32,
            [115, 116, 117],
            "insert-zero",
            "h6-CurrencyAllNull-insert-zero.accdb",
            "5e59fb5666b9f51eae29233e209ec32228f2c6ab6ce7c1455671c7be3b71ff68",
        ),
        (
            6_u8,
            "CurrencyAllNull",
            113_u32,
            118_u32,
            [115, 116, 117],
            "delete-all",
            "h6-CurrencyAllNull-delete-all.accdb",
            "3915d381b9cbb0d566f4b1e6beb2d01c9b0bd5574a1582e81e7d3b845e2e8e22",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            120_u32,
            126_u32,
            [122, 123, 124],
            "before",
            "h6-Int64WidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            120_u32,
            126_u32,
            [122, 123, 124],
            "insert-null",
            "h6-Int64WidePrimary-insert-null.accdb",
            "7b19ebe1be715123235e0b6e64fbfa86798843a19229dce40980756dfd3926a9",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            120_u32,
            126_u32,
            [122, 123, 124],
            "insert-zero",
            "h6-Int64WidePrimary-insert-zero.accdb",
            "b10032b46b0e153c2b5541861273b4839815b882f79ebc4277a164cbda1b756c",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            120_u32,
            126_u32,
            [122, 123, 124],
            "delete-all",
            "h6-Int64WidePrimary-delete-all.accdb",
            "2e7bb9c3d829b61eb28c3b9aad9d6015ed2616bf90809db182c6b34fb2d64240",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            128_u32,
            133_u32,
            [130, 131, 132],
            "before",
            "h6-CurrencyWidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            128_u32,
            133_u32,
            [130, 131, 132],
            "insert-null",
            "h6-CurrencyWidePrimary-insert-null.accdb",
            "3fdd0540be9147a3a5d0353194f1f37ff29b7e5351b31ccf1af79bd50cd625e9",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            128_u32,
            133_u32,
            [130, 131, 132],
            "insert-zero",
            "h6-CurrencyWidePrimary-insert-zero.accdb",
            "c90921bd6afa891d7152403214b814ba749ce6183c1424ad539f2ecdf3fe2e19",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            128_u32,
            133_u32,
            [130, 131, 132],
            "delete-all",
            "h6-CurrencyWidePrimary-delete-all.accdb",
            "cf9df1d97646745e0e12116f825c377f771835f1ac20a7d6c3e5129bee11899a",
        ),
        (
            6_u8,
            "Int64StableIds",
            135_u32,
            140_u32,
            [137, 138, 139],
            "before",
            "h6-Int64StableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "Int64StableIds",
            135_u32,
            140_u32,
            [137, 138, 139],
            "insert-null",
            "h6-Int64StableIds-insert-null.accdb",
            "9a0675d4e31cdbc14aeb6ae04e9cd6753b53493637f319883c8e3b5ed42fb1fa",
        ),
        (
            6_u8,
            "Int64StableIds",
            135_u32,
            140_u32,
            [137, 138, 139],
            "insert-zero",
            "h6-Int64StableIds-insert-zero.accdb",
            "1e233cc94264a93f13a6e60f7fe1613e7a212be82a24f5e1b5daf0eee8d91527",
        ),
        (
            6_u8,
            "Int64StableIds",
            135_u32,
            140_u32,
            [137, 138, 139],
            "delete-all",
            "h6-Int64StableIds-delete-all.accdb",
            "dfa822d3138f84991a4587f22a32aff123c0507d4701c0d87afc67df68059ac6",
        ),
        (
            6_u8,
            "CurrencyStableIds",
            142_u32,
            147_u32,
            [144, 145, 146],
            "before",
            "h6-CurrencyStableIds-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
        ),
        (
            6_u8,
            "CurrencyStableIds",
            142_u32,
            147_u32,
            [144, 145, 146],
            "insert-null",
            "h6-CurrencyStableIds-insert-null.accdb",
            "d4d3fb123cb682a15fdeb2b824fd7680acb5dae6602552f592b85b334d1b81b1",
        ),
        (
            6_u8,
            "CurrencyStableIds",
            142_u32,
            147_u32,
            [144, 145, 146],
            "insert-zero",
            "h6-CurrencyStableIds-insert-zero.accdb",
            "4144c1119e3ffff77163482fd094174988acf0486218d9a108cce2ea7fa6951e",
        ),
        (
            6_u8,
            "CurrencyStableIds",
            142_u32,
            147_u32,
            [144, 145, 146],
            "delete-all",
            "h6-CurrencyStableIds-delete-all.accdb",
            "5db2133516ed89e43123081aa12d63f639194febe27b413b814fb7525cb72310",
        ),
    ] {
        let original =
            fs::read(old.join(format!("h{header}-integral-extra-original.accdb"))).unwrap();
        assert_eq!(
            digest(&original),
            if header == 5 {
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e"
            } else {
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7"
            }
        );
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(digest(&raw), hash);
        assert_eq!(
            raw,
            literal(&original, table, definition, data, roots, recipe)
        );
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let active = rows(table, recipe);
        let expected = active
            .iter()
            .map(|row| values(table, *row))
            .collect::<Vec<_>>();
        let snapshot =
            read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(snapshot.rows(), expected);
        assert_eq!(snapshot.schema().definition_page(), definition);
        for (position, field) in snapshot.schema().columns().iter().enumerate() {
            assert_eq!(field.number(), position);
            let at = definition as usize * 4096 + 99 + 25 * position + 9;
            assert_eq!(
                u16_at(&raw, at),
                if table.ends_with("StableIds") {
                    [0, 3, 4][position]
                } else {
                    position as u16
                }
            );
        }
        let constraints =
            check_table_constraints(&mut image.reader(), SCOPE, table, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(constraints.rows(), active.len());
        assert!(constraints.relationships().is_empty());
        let checked =
            check_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(checked.rows(), active.len());
        assert_eq!(checked.indexes().len(), 6);
        for (position, checked) in checked.indexes().iter().enumerate() {
            let physical = position / 2;
            let index = checked.index();
            assert_eq!(checked.status(), IndexCheckStatus::Checked);
            assert_eq!(index.name(), ALIASES[position]);
            assert_eq!(index.root_page(), roots[physical]);
            assert_eq!(index.columns()[0].descending(), physical == 2);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                table,
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), history(table, recipe, physical));
            let mut sorted = active.clone();
            order(&mut sorted, physical);
            let ordered = read_table_index(
                &mut image.reader(),
                SCOPE,
                table,
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                ordered.rows(),
                sorted
                    .into_iter()
                    .map(|row| values(table, row))
                    .collect::<Vec<_>>()
            );
        }
        if table.ends_with("StableIds") {
            assert_eq!(
                super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                    .err()
                    .unwrap(),
                EditError::Unsupported(WriteBlocker::ActiveContent)
            );
        } else {
            super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                .unwrap();
            image
                .check_native_ordinary_fixed_table(table, &mut budget(), &|| false)
                .unwrap();
        }
        assert_eq!(image.as_bytes(), raw);
    }
}
