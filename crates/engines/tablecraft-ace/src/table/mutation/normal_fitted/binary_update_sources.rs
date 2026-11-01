//! Independent original/request byte reconstruction and read-only source checks.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(204);
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
const KEYS: [Option<&[u8]>; 13] = [
    None,
    Some(b""),
    Some(b"\x00"),
    Some(b"\xff"),
    Some(b"\x00\x00"),
    Some(b"\x00\x01"),
    Some(b"\x00\x01\x02\x03\x04\x05\x06"),
    Some(b"\x00\x01\x02\x03\x04\x05\x06\x07"),
    Some(b"\x00\x01\x02\x03\x04\x05\x06\x07\x08"),
    Some(b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f"),
    Some(b"\x00\x00\xff\x00"),
    Some(b"\xff\x00\xff\x00\xff\x00\xff\x00"),
    Some(b"\x00"),
];

#[derive(Clone)]
struct Row {
    slot: usize,
    id: i32,
    key: Option<Vec<u8>>,
    marker: i32,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn original_rows() -> Vec<Row> {
    KEYS.iter()
        .enumerate()
        .map(|(slot, key)| Row {
            slot,
            id: slot as i32 + 1,
            key: key.map(Vec::from),
            marker: 17 * (slot as i32 + 1),
        })
        .collect()
}

fn values(row: &Row) -> Vec<Value> {
    vec![
        Value::Int32(row.id),
        row.key.clone().map_or(Value::Null, Value::Binary),
        Value::Int32(row.marker),
    ]
}

fn key(row: &Row, physical: usize) -> Vec<u8> {
    if physical == 0 {
        let mut bytes = vec![127];
        bytes.extend_from_slice(&((row.id as u32) ^ 0x8000_0000).to_be_bytes());
        return bytes;
    }

    let descending = physical == 2;
    let Some(value) = &row.key else {
        return vec![if descending { 255 } else { 0 }];
    };

    let mut bytes = vec![if descending { 128 } else { 127 }];
    for (number, chunk) in value.chunks(8).enumerate() {
        let continued = (number + 1) * 8 < value.len();
        let mut segment = [0_u8; 8];
        segment[..chunk.len()].copy_from_slice(chunk);
        if descending {
            for byte in &mut segment {
                *byte = !*byte;
            }
        }
        bytes.extend_from_slice(&segment);
        let tag = if continued { 9 } else { chunk.len() as u8 };
        bytes.push(if descending && !continued { !tag } else { tag });
    }
    bytes
}

fn leaf(original: &[u8], after: &mut [u8], root: u32, data: u32, physical: usize, rows: &[Row]) {
    let mut entries = rows
        .iter()
        .map(|row| (key(row, physical), (data << 8) | row.slot as u32))
        .collect::<Vec<_>>();
    entries.sort();
    let mut bitmap = [0_u8; 453];
    let mut payload = Vec::new();

    for (key, reference) in entries {
        payload.extend_from_slice(&key);
        payload.extend_from_slice(&reference.to_be_bytes());
        let end = payload.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }

    let page = root as usize * 4096;
    after[page + 2..page + 4].copy_from_slice(&((3616 - payload.len()) as u16).to_le_bytes());
    after[page + 27..page + 480].copy_from_slice(&bitmap);
    after[page + 480..page + 480 + payload.len()].copy_from_slice(&payload);
    assert_eq!(
        &after[page + 480 + payload.len()..page + 4096],
        &original[page + 480 + payload.len()..page + 4096]
    );
}

fn replacement(recipe: &str, slot: usize) -> Option<Option<&'static [u8]>> {
    match (recipe, slot) {
        ("same-length-new-family", 3) => Some(Some(&[0xfe])),
        ("shrink-to-existing", 9) => Some(Some(&[0, 1, 2, 3, 4, 5, 6, 7])),
        ("null-to-empty", 0) => Some(Some(&[])),
        ("empty-to-null", 1) | ("present-to-null", 8) => Some(None),
        ("duplicate-to-new" | "duplicate-merge", 2) | ("duplicate-merge", 12) => {
            Some(Some(&[0x55]))
        }
        _ => None,
    }
}

fn final_rows(recipe: &str) -> Vec<Row> {
    original_rows()
        .into_iter()
        .map(|mut row| {
            if let Some(value) = replacement(recipe, row.slot) {
                row.key = value.map(Vec::from);
            }
            row
        })
        .collect()
}

fn literal(original: &[u8], data: u32, roots: [u32; 3], recipe: &str) -> Vec<u8> {
    let mut after = original.to_vec();
    let page = data as usize * 4096;
    for row in original_rows() {
        let Some(value) = replacement(recipe, row.slot) else {
            continue;
        };
        let start_at = page + 14 + row.slot * 2;
        let start =
            u16::from_le_bytes(original[start_at..start_at + 2].try_into().unwrap()) as usize;
        let end = if row.slot == 0 {
            4096
        } else {
            u16::from_le_bytes(original[start_at - 2..start_at].try_into().unwrap()) as usize
        };
        let payload = value.unwrap_or(&[]);
        assert!(payload.len() <= row.key.as_ref().map_or(0, Vec::len));
        after[page + start + 10..page + start + 10 + payload.len()].copy_from_slice(payload);
        after[page + end - 7..page + end - 5]
            .copy_from_slice(&((10 + payload.len()) as u16).to_le_bytes());
        after[page + end - 1] = if value.is_some() { 7 } else { 5 };
        assert_eq!(
            &after[page + start + 10 + payload.len()..page + end - 7],
            &original[page + start + 10 + payload.len()..page + end - 7]
        );
    }
    let rows = final_rows(recipe);
    for physical in [1, 2] {
        leaf(original, &mut after, roots[physical], data, physical, &rows);
    }
    after
}

#[test]
#[ignore = "requires16 original/literal fitting Binary UPDATE source views; no producer"]
fn fitting_binary_updates_preserve_source_capacity_history_and_complete_membership() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_UPDATE_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    let recipes = [
        "before",
        "same-length-new-family",
        "shrink-to-existing",
        "null-to-empty",
        "empty-to-null",
        "duplicate-to-new",
        "present-to-null",
        "duplicate-merge",
    ];
    for header in [5_u8, 6] {
        let before_name = format!("h{header}-binary-before.accdb");
        let original = fs::read(dir.join(before_name)).unwrap();
        assert_eq!(
            digest(&original),
            if header == 5 {
                "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398"
            } else {
                "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd"
            }
        );
        let (td, data, roots) = if header == 5 {
            (89_u32, 94_u32, [91, 92, 93])
        } else {
            (106_u32, 111_u32, [108, 109, 110])
        };
        for recipe in recipes {
            let raw = fs::read(dir.join(format!("h{header}-binary-{recipe}.accdb"))).unwrap();
            assert_eq!(raw, literal(&original, data, roots, recipe));
            let image =
                NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let states = final_rows(recipe);
            let snapshot = read_table(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                snapshot.rows(),
                states.iter().map(values).collect::<Vec<_>>()
            );
            assert_eq!(snapshot.schema().definition_page(), td);
            assert_eq!(snapshot.schema().columns().len(), 3);
            for (position, field) in snapshot.schema().columns().iter().enumerate() {
                assert_eq!(field.number(), position);
                assert_eq!(field.native_type(), if position == 1 { 9 } else { 4 });
                assert_eq!(field.storage_length(), if position == 1 { 16 } else { 4 });
                assert_eq!(field.fixed(), position != 1);
            }
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(constraints.rows(), states.len());
            assert!(constraints.relationships().is_empty());
            let checked = check_table(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), states.len());
            assert_eq!(checked.indexes().len(), 6);
            let strict = crate::table::indexes::check_table_retained(
                &mut image.reader(),
                SCOPE,
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                strict
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            for (position, checked) in checked.indexes().iter().enumerate() {
                let physical = position / 2;
                let index = checked.index();
                assert_eq!(checked.status(), IndexCheckStatus::Checked);
                assert_eq!(index.name(), ALIASES[position]);
                assert_eq!(index.root_page(), roots[physical]);
                assert_eq!(index.columns().len(), 1);
                assert_eq!(index.columns()[0].number(), usize::from(physical != 0));
                assert_eq!(index.columns()[0].descending(), physical == 2);
                let history = if physical == 0 { 13 } else { 12 };
                let counter = inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "BinaryKeys",
                    index.name(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(counter.value(), history);
                let mut ordered = states.clone();
                ordered.sort_by_key(|row| (key(row, physical), (data << 8) | row.slot as u32));
                let read = read_table_index(
                    &mut image.reader(),
                    SCOPE,
                    "BinaryKeys",
                    index.name(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(read.rows(), ordered.iter().map(values).collect::<Vec<_>>());
            }
            assert_eq!(image.as_bytes(), raw);
        }
    }
}
