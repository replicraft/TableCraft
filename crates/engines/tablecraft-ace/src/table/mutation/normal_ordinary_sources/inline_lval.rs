//! Independent inline Memo/OLE source reads only; no insertion planner or serializer.
use crate::{
    LongValueCheckStatus, NativeImage, check_table_constraints, check_table_long_values,
    inspect_table_indexes, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(text, 16).unwrap()
        })
        .collect()
}

fn new_values(recipe: &str) -> Vec<Value> {
    let (memo, blob) = match recipe {
        "ascii" => (Some("new".to_owned()), Some(vec![0, 1, 254, 255])),
        "raw-unicode-nul" => (Some("β\0雪".to_owned()), Some(vec![165, 0, 90])),
        "empty" => (Some(String::new()), Some(Vec::new())),
        "null" => (None, None),
        "memo-null-ole-empty" => (None, Some(Vec::new())),
        "memo-empty-ole-null" => (Some(String::new()), None),
        "compressed-limit" => (Some("a".repeat(62)), Some((0..64).collect())),
        "raw-limit" => (Some("β".repeat(32)), Some((0..64).rev().collect())),
        _ => panic!("unknown literal recipe"),
    };
    vec![
        Value::Int32(2),
        memo.map(Value::Text).unwrap_or(Value::Null),
        blob.map(Value::Binary).unwrap_or(Value::Null),
    ]
}

fn check_view(raw: &[u8], header: u8, recipe: &str) {
    let definition = if header == 5 { 89 } else { 106 };
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    let mut expected = vec![vec![
        Value::Int32(1),
        Value::Text("old".into()),
        Value::Binary(vec![199, 0, 255]),
    ]];
    if recipe != "original" {
        expected.push(new_values(recipe));
    }
    assert_eq!(table.rows(), expected);
    let fields = table.schema().columns();
    assert_eq!(fields.len(), 3);
    for (field, name, kind, flags, extended, width) in [
        (&fields[0], "Id", 4, 3, 0, 4),
        (&fields[1], "MemoValue", 12, 2, 1, 0),
        (&fields[2], "BlobValue", 11, 2, 0, 0),
    ] {
        assert_eq!(
            (
                field.name(),
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (name, kind, flags, extended, width)
        );
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), expected.len());
    assert!(constraints.relationships().is_empty());
    assert!(
        inspect_table_indexes(
            &mut image.reader(),
            SCOPE,
            "InlineValues",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .is_empty()
    );
    let long = check_table_long_values(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(long.rows(), expected.len());
    assert_eq!(long.columns().len(), 2);
    for (position, column) in long.columns().iter().enumerate() {
        assert_eq!(column.column_number(), position + 1);
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        let nulls = expected
            .iter()
            .filter(|row| row[position + 1] == Value::Null)
            .count();
        assert_eq!(
            (
                column.values().nulls(),
                column.values().inline(),
                column.values().single(),
                column.values().chained()
            ),
            (nulls, expected.len() - nulls, 0, 0)
        );
        assert_eq!(
            (
                column.owned_pages(),
                column.candidate_pages(),
                column.owned_rows(),
                column.distinct_chunks()
            ),
            (0, 0, 0, 0)
        );
    }
    super::super::normal_ordinary::capture(&image, "InlineValues", &mut budget(), &|| false)
        .unwrap();
    image
        .check_native_ordinary_fixed_table("InlineValues", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires independently authored inline Memo/OLE source directory"]
fn inline_memo_ole_literal_sources_read_and_preserve_full_ordinary_closure() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INLINE_LVAL_INSERT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    assert_eq!(
        digest(&fs::read(directory.join("manifest.json")).unwrap()),
        "7e9d7fefce45a5a524a4d3f960a5d52cd09e0c2a0bcbd8425db9a0c28c27b2cc"
    );
    let mut originals = Vec::new();
    for (header, recipe, name, length, hash, record_hex) in [
        (
            5_u8,
            "original",
            "h5-inline-insert-original.accdb",
            380928_usize,
            "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
            "",
        ),
        (
            5_u8,
            "ascii",
            "h5-inline-insert-ascii.accdb",
            380928_usize,
            "1be970bbd7511e3b4de48c325f7bc798c53544cc8031623a9339156fa63c85c4",
            "030002000000050000800000000000000000fffe6e65770400008000000000000000000001feff270017000600020007",
        ),
        (
            5_u8,
            "raw-unicode-nul",
            "h5-inline-insert-raw-unicode-nul.accdb",
            380928_usize,
            "0bb649e78cdb33e30618db7d27230c68f9a6702dd0d0c32a3d81bc1b44da1fb3",
            "030002000000060000800000000000000000b2030000ea96030000800000000000000000a5005a270018000600020007",
        ),
        (
            5_u8,
            "empty",
            "h5-inline-insert-empty.accdb",
            380928_usize,
            "4551ad4e85716cbba3407deeb21da8d9efc44e3932f8cf02e377fda29add5479",
            "0300020000000000008000000000000000000000008000000000000000001e0012000600020007",
        ),
        (
            5_u8,
            "null",
            "h5-inline-insert-null.accdb",
            380928_usize,
            "2d5e1407721a243ec8410652cf8d966a3e1c3b58b93d22744e00eb0e4e69f105",
            "030002000000060006000600020001",
        ),
        (
            5_u8,
            "memo-null-ole-empty",
            "h5-inline-insert-memo-null-ole-empty.accdb",
            380928_usize,
            "f895177239753d93f1f280e680f631976f3ff0029f00c27890a0c3f6032580be",
            "030002000000000000800000000000000000120006000600020005",
        ),
        (
            5_u8,
            "memo-empty-ole-null",
            "h5-inline-insert-memo-empty-ole-null.accdb",
            380928_usize,
            "3034a22ac1fd3487bfdad937d6737c0f08805fd0c474631044b10652be429093",
            "030002000000000000800000000000000000120012000600020003",
        ),
        (
            5_u8,
            "compressed-limit",
            "h5-inline-insert-compressed-limit.accdb",
            380928_usize,
            "0323ffed5fcbc26069fee9ed3d44a6b8077997f15f40dff9835deeb83fe8321a",
            "030002000000400000800000000000000000fffe6161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161400000800000000000000000000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f9e0052000600020007",
        ),
        (
            5_u8,
            "raw-limit",
            "h5-inline-insert-raw-limit.accdb",
            380928_usize,
            "8decd1b29fbd86ac1cdfab1a1b632a57388ff9a6a573a99aaee2c9b8f2b35f78",
            "030002000000400000800000000000000000b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b2034000008000000000000000003f3e3d3c3b3a393837363534333231302f2e2d2c2b2a292827262524232221201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201009e0052000600020007",
        ),
        (
            6_u8,
            "original",
            "h6-inline-insert-original.accdb",
            450560_usize,
            "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
            "",
        ),
        (
            6_u8,
            "ascii",
            "h6-inline-insert-ascii.accdb",
            450560_usize,
            "906dfa24fa024d2de995d2fa6f1fd75c80a42a5ccdf13ff5db9ff6ede81b2cb9",
            "030002000000050000800000000000000000fffe6e65770400008000000000000000000001feff270017000600020007",
        ),
        (
            6_u8,
            "raw-unicode-nul",
            "h6-inline-insert-raw-unicode-nul.accdb",
            450560_usize,
            "09bc773b55a19b14c1bfa9fb6d2a88569338a588e06e4410a63b07ccc2b7766d",
            "030002000000060000800000000000000000b2030000ea96030000800000000000000000a5005a270018000600020007",
        ),
        (
            6_u8,
            "empty",
            "h6-inline-insert-empty.accdb",
            450560_usize,
            "9dce1c1035d14cdc8b37779482c561b272a04cbb1a4240399d3906cf492fe06b",
            "0300020000000000008000000000000000000000008000000000000000001e0012000600020007",
        ),
        (
            6_u8,
            "null",
            "h6-inline-insert-null.accdb",
            450560_usize,
            "d671add198221de944a1863db5dd09aac97a2d4cb1364d7f80782a12a89dd86c",
            "030002000000060006000600020001",
        ),
        (
            6_u8,
            "memo-null-ole-empty",
            "h6-inline-insert-memo-null-ole-empty.accdb",
            450560_usize,
            "5c5382b3f61b3d1465a9c47b475e16e42185b976405e18ac3b4a54deeb485011",
            "030002000000000000800000000000000000120006000600020005",
        ),
        (
            6_u8,
            "memo-empty-ole-null",
            "h6-inline-insert-memo-empty-ole-null.accdb",
            450560_usize,
            "e3a81cdf0439c76fefecdfdd22f90ad8426b61ee1571b8841d8068fcfcfee489",
            "030002000000000000800000000000000000120012000600020003",
        ),
        (
            6_u8,
            "compressed-limit",
            "h6-inline-insert-compressed-limit.accdb",
            450560_usize,
            "a6a6384ebae73f49c9cc7fffdfa00f2d365f4c980080cbacaf9ca9c88cade981",
            "030002000000400000800000000000000000fffe6161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161400000800000000000000000000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f9e0052000600020007",
        ),
        (
            6_u8,
            "raw-limit",
            "h6-inline-insert-raw-limit.accdb",
            450560_usize,
            "00aa4de17b9924cc8cd1c6135d76eaa229457de00968eba4313051be458cc538",
            "030002000000400000800000000000000000b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b203b2034000008000000000000000003f3e3d3c3b3a393837363534333231302f2e2d2c2b2a292827262524232221201f1e1d1c1b1a191817161514131211100f0e0d0c0b0a090807060504030201009e0052000600020007",
        ),
    ] {
        let raw = fs::read(directory.join(name)).unwrap();
        assert_eq!(raw.len(), length);
        assert_eq!(digest(&raw), hash);
        if recipe == "original" {
            originals.push(raw.clone());
        } else {
            let before = &originals[usize::from(header - 5)];
            let definition = if header == 5 { 89 } else { 106 };
            let data = if header == 5 { 91 } else { 108 };
            let record = bytes(record_hex);
            let start = 4049 - record.len();
            let mut literal = before.clone();
            literal[definition * 4096 + 16..definition * 4096 + 20]
                .copy_from_slice(&2_u32.to_le_bytes());
            let base = data * 4096;
            literal[base + 2..base + 4].copy_from_slice(&((start - 18) as u16).to_le_bytes());
            literal[base + 12..base + 14].copy_from_slice(&2_u16.to_le_bytes());
            literal[base + 16..base + 18].copy_from_slice(&(start as u16).to_le_bytes());
            literal[base + start..base + 4049].copy_from_slice(&record);
            assert_eq!(
                raw, literal,
                "{recipe}: every nonpatch byte, original row, property and map is retained"
            );
        }
        check_view(&raw, header, recipe);
    }
}
