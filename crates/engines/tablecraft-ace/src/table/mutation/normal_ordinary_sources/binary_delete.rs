//! Independent original/request byte reconstruction and read-only source checks.
use crate::{
    EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table, check_table_constraints,
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

fn selected(recipe: &str) -> &'static [usize] {
    match recipe {
        "before" => &[],
        "delete-null" => &[0],
        "delete-empty" => &[1],
        "delete-one-duplicate" => &[2],
        "delete-both-duplicates" => &[2, 12],
        "delete-segments" => &[6, 7, 8, 9],
        "delete-all" => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        _ => panic!("outside frozen deletion laws"),
    }
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

fn literal(original: &[u8], td: u32, data: u32, roots: [u32; 3], recipe: &str) -> Vec<u8> {
    let mut after = original.to_vec();
    let selected = selected(recipe);
    let rows = original_rows()
        .into_iter()
        .filter(|row| !selected.contains(&row.slot))
        .collect::<Vec<_>>();
    let page = data as usize * 4096;
    for slot in selected {
        let at = page + 14 + 2 * slot;
        let word = u16::from_le_bytes(original[at..at + 2].try_into().unwrap());
        after[at..at + 2].copy_from_slice(&(word | 0xc000).to_le_bytes());
    }
    let definition = td as usize * 4096;
    after[definition + 16..definition + 20].copy_from_slice(&(rows.len() as u32).to_le_bytes());
    for (physical, root) in roots.iter().enumerate() {
        leaf(original, &mut after, *root, data, physical, &rows);
    }
    after
}

#[test]
#[ignore = "requires14 independent Binary DELETE views and112 read-only SDK reports"]
fn binary_delete_original_and_literal_after_views_have_exact_membership_and_preservation() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_DELETE_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    assert_eq!(
        digest(&fs::read(dir.join("manifest.json")).unwrap()),
        "d055d976fde94c7f1c248d57091211029cbf6ab837fbc9a12fe175e97943810a"
    );
    for (header, recipe, filename, hash) in [
        (
            5_u8,
            "before",
            "h5-binary-before.accdb",
            "57c267fa365c2ff49e3ededdcfef7a6cde2f721642266c57f1ed7c7e44175398",
        ),
        (
            5_u8,
            "delete-null",
            "h5-binary-delete-null.accdb",
            "5db3be2121ac9e6f5ceff8cf0cf2156078d6a0b6907c6ea11872e2330cefa988",
        ),
        (
            5_u8,
            "delete-empty",
            "h5-binary-delete-empty.accdb",
            "0406d9549bd03bc4c85a79cfe3645aabdf05cb28de282bec51f3a13e29ada5e8",
        ),
        (
            5_u8,
            "delete-one-duplicate",
            "h5-binary-delete-one-duplicate.accdb",
            "7a661d62293d9c6ecbc837d9f2b71a3af43b45f2c656fe25498572624bb8c7f7",
        ),
        (
            5_u8,
            "delete-both-duplicates",
            "h5-binary-delete-both-duplicates.accdb",
            "e5303c44e676afb7c8127c5f3ad946a0c5886abcd08f536ab8b064fee1228976",
        ),
        (
            5_u8,
            "delete-segments",
            "h5-binary-delete-segments.accdb",
            "8780297c0ae0a370e49cdf03495798e018bcf6ad286d983e865b63f287f09424",
        ),
        (
            5_u8,
            "delete-all",
            "h5-binary-delete-all.accdb",
            "8bdcae4fec767c8b9720d099d6464be895bf1eadcfef7c8680f1aa4e54f93997",
        ),
        (
            6_u8,
            "before",
            "h6-binary-before.accdb",
            "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
        ),
        (
            6_u8,
            "delete-null",
            "h6-binary-delete-null.accdb",
            "66fa70f2a08c94dd4f63637032ed0a8ecb09df6d28616b906362ef181cd78acf",
        ),
        (
            6_u8,
            "delete-empty",
            "h6-binary-delete-empty.accdb",
            "ce11d6e37a07de416f6aa44be8cb5a20d4a1515f615d1dc4350a82d915e5789b",
        ),
        (
            6_u8,
            "delete-one-duplicate",
            "h6-binary-delete-one-duplicate.accdb",
            "f07291b02758943761eee2b305af722e354de6fd5f3d7f8c9dd9b248e91dad4f",
        ),
        (
            6_u8,
            "delete-both-duplicates",
            "h6-binary-delete-both-duplicates.accdb",
            "5bfa53e54edc59e3b262b68c0bc1011e3dc20b082eab04b0a421556768022893",
        ),
        (
            6_u8,
            "delete-segments",
            "h6-binary-delete-segments.accdb",
            "76e3537befe14e5088beb700ba3d5b4c6124f9fe70a54b220e204968372c56ff",
        ),
        (
            6_u8,
            "delete-all",
            "h6-binary-delete-all.accdb",
            "aafffecbb1e4a7e6607cac67a6ac9e4f7b97a57cd52747f01f358ba01cb28401",
        ),
    ] {
        let original = fs::read(dir.join(format!("h{header}-binary-before.accdb"))).unwrap();
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
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(digest(&raw), hash);
        assert_eq!(raw, literal(&original, td, data, roots, recipe));
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let states = original_rows()
            .into_iter()
            .filter(|row| !selected(recipe).contains(&row.slot))
            .collect::<Vec<_>>();
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
        assert_eq!(
            image
                .check_native_ordinary_fixed_table("BinaryKeys", &mut budget(), &|| false)
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::ActiveContent)
        );
        assert_eq!(image.as_bytes(), raw);
    }
}
