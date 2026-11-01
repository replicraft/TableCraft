//! Read-only literal default/NULL/override sources; no INSERT or resolver call.
use crate::{IndexCheckStatus, NativeImage, check_table, read_table, read_table_index};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(181);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn original_values() -> Vec<Value> {
    vec![
        Value::Int32(1),
        Value::Int32(-1),
        Value::Text("original".to_owned()),
        Value::Boolean(false),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Null,
    ]
}

fn added_values(recipe: &str) -> Vec<Value> {
    match recipe {
        "omitted-constants" => vec![
            Value::Int32(24),
            Value::Int32(7),
            Value::Text("seed".to_owned()),
            Value::Boolean(true),
            Value::Currency(Currency::from_scaled(-10001)),
            Value::Null,
        ],
        "explicit-null" => vec![
            Value::Int32(25),
            Value::Null,
            Value::Null,
            Value::Boolean(false),
            Value::Null,
            Value::Null,
        ],
        "all-overrides" => vec![
            Value::Int32(26),
            Value::Int32(-9),
            Value::Text("override".to_owned()),
            Value::Boolean(false),
            Value::Currency(Currency::from_scaled(12345)),
            Value::Int32(99),
        ],
        _ => panic!("Unlisted independent source recipe"),
    }
}

fn literal(before: &[u8], definition: usize, data: usize, root: usize, recipe: &str) -> Vec<u8> {
    let (id, score, money, optional, caption, mask) = match recipe {
        "omitted-constants" => (24_i32, 7_i32, -10001_i64, 0_i32, "seed", 0x1f_u8),
        "explicit-null" => (25, 0, 0, 0, "", 0x01),
        "all-overrides" => (26, -9, 12345, 99, "override", 0x37),
        _ => panic!("Unlisted independent source recipe"),
    };
    let mut record = vec![6, 0];
    record.extend_from_slice(&id.to_le_bytes());
    record.extend_from_slice(&score.to_le_bytes());
    record.extend_from_slice(&money.to_le_bytes());
    record.extend_from_slice(&optional.to_le_bytes());
    assert_eq!(record.len(), 22);
    for unit in caption.encode_utf16() {
        record.extend_from_slice(&unit.to_le_bytes());
    }
    let end = record.len() as u16;
    record.extend_from_slice(&end.to_le_bytes());
    record.extend_from_slice(&[22, 0, 1, 0, mask]);
    let start = 4051 - record.len();
    let mut after = before.to_vec();
    let base = data * 4096;
    after[base + start..base + 4051].copy_from_slice(&record);
    after[base + 16..base + 18].copy_from_slice(&(start as u16).to_le_bytes());
    after[base + 12..base + 14].copy_from_slice(&2_u16.to_le_bytes());
    after[base + 2..base + 4].copy_from_slice(&((start - 18) as u16).to_le_bytes());
    after[definition * 4096 + 16..definition * 4096 + 20].copy_from_slice(&2_u32.to_le_bytes());
    after[definition * 4096 + 67..definition * 4096 + 71].copy_from_slice(&2_u32.to_le_bytes());
    let mut wire = vec![0x7f];
    wire.extend_from_slice(&0x8000_0001_u32.to_be_bytes());
    wire.extend_from_slice(&((data as u32) << 8).to_be_bytes());
    wire.push(0x7f);
    wire.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
    wire.extend_from_slice(&(((data as u32) << 8) | 1).to_be_bytes());
    let mut bitmap = vec![0_u8; 453];
    bitmap[1] = 2;
    bitmap[2] = 4;
    let leaf = root * 4096;
    after[leaf + 2..leaf + 4].copy_from_slice(&3598_u16.to_le_bytes());
    after[leaf + 27..leaf + 480].copy_from_slice(&bitmap);
    after[leaf + 480..leaf + 498].copy_from_slice(&wire);
    after
}

#[test]
#[ignore = "requires eight independently constructed/read-qualified ordinary default source views"]
fn literal_default_omission_and_supplied_null_images_preserve_original_properties_and_bytes() {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_NORMAL_DEFAULT_AFTER_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut originals = Vec::new();
    for (header, length, digest, definition, data, root) in [
        (
            5,
            446464,
            "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
            89,
            92,
            91,
        ),
        (
            6,
            520192,
            "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
            106,
            109,
            108,
        ),
    ] {
        let before = fs::read(directory.join(format!("h{header}-default-original.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(hash(&before), digest);
        originals.push((header, definition, data, root, before));
    }
    for (header, definition, data, root, before) in originals {
        for recipe in ["omitted-constants", "explicit-null", "all-overrides"] {
            let after =
                fs::read(directory.join(format!("h{header}-default-{recipe}.accdb"))).unwrap();
            assert_eq!(after, literal(&before, definition, data, root, recipe));
            let image =
                NativeImage::capture(&mut Cursor::new(&after), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let source = crate::table::mutation::normal_ordinary::capture(
                &image,
                "Defaults",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected = vec![original_values(), added_values(recipe)];
            assert_eq!(source.table.rows(), expected);
            assert_eq!(source.records.len(), 2);
            let checked = check_table(
                &mut image.reader(),
                SCOPE,
                "Defaults",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 2);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let rows = read_table(
                &mut image.reader(),
                SCOPE,
                "Defaults",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), expected);
            for name in ["PrimaryID", "PrimaryAlias"] {
                let rows = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "Defaults",
                    name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(rows.rows(), expected);
            }
            assert_eq!(image.as_bytes(), after);
        }
    }
}
