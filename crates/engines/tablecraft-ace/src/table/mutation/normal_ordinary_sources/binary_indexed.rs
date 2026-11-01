//! Independent original/request byte reconstruction and read-only source checks.
use crate::{
    EditError, IndexCheckStatus, NativeImage, WriteBlocker, check_table, check_table_constraints,
    inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};
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
const SEVEN: &[u8] = b"\x00\x01\x02\x03\x04\x05\xff";
const NINE: &[u8] = b"\xff\x00\xff\x00\xff\x00\xff\x00\x00";
const MAXIMUM: &[u8] = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\xff";

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

fn requests(recipe: &str) -> Vec<Row> {
    let mut rows = match recipe {
        "before" => Vec::new(),
        "insert-null" => vec![(14, None, 238)],
        "insert-empty" => vec![(14, Some(&b""[..]), 238)],
        "insert-existing" => vec![(14, Some(&b"\x00"[..]), 238)],
        "insert-seven" => vec![(14, Some(SEVEN), 238)],
        "insert-nine" => vec![(14, Some(NINE), 238)],
        "insert-maximum" => vec![(14, Some(MAXIMUM), 238)],
        "batch" | "batch-reverse" => vec![
            (14, None, 238),
            (15, Some(&b""[..]), 255),
            (16, Some(&b"\x00"[..]), 272),
            (17, Some(NINE), 289),
            (18, Some(MAXIMUM), 306),
        ],
        _ => unreachable!(),
    };
    if recipe == "batch-reverse" {
        rows.reverse();
    }

    rows.into_iter()
        .enumerate()
        .map(|(number, (id, key, marker))| Row {
            slot: 13 + number,
            id,
            key: key.map(Vec::from),
            marker,
        })
        .collect()
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

fn record(row: &Row) -> Vec<u8> {
    let mut bytes = 3_u16.to_le_bytes().to_vec();
    bytes.extend_from_slice(&row.id.to_le_bytes());
    bytes.extend_from_slice(&row.marker.to_le_bytes());
    if let Some(payload) = &row.key {
        bytes.extend_from_slice(payload);
    }

    let end = bytes.len() as u16;
    bytes.extend_from_slice(&end.to_le_bytes());
    bytes.extend_from_slice(&10_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.push(if row.key.is_none() { 5 } else { 7 });
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
    let mut rows = original_rows();
    let requested = requests(recipe);
    let page = data as usize * 4096;
    let mut frontier = 3816;

    for row in &requested {
        let bytes = record(row);
        frontier -= bytes.len();
        after[page + frontier..page + frontier + bytes.len()].copy_from_slice(&bytes);
        after[page + 14 + 2 * row.slot..page + 16 + 2 * row.slot]
            .copy_from_slice(&(frontier as u16).to_le_bytes());
    }
    rows.extend(requested);

    after[page + 2..page + 4]
        .copy_from_slice(&((frontier - 14 - 2 * rows.len()) as u16).to_le_bytes());
    after[page + 12..page + 14].copy_from_slice(&(rows.len() as u16).to_le_bytes());

    let definition = td as usize * 4096;
    after[definition + 16..definition + 20].copy_from_slice(&(rows.len() as u32).to_le_bytes());

    for (physical, root) in roots.iter().enumerate() {
        leaf(original, &mut after, *root, data, physical, &rows);
        let old = original_rows()
            .iter()
            .map(|row| key(row, physical))
            .collect::<BTreeSet<_>>();
        let new = rows
            .iter()
            .map(|row| key(row, physical))
            .collect::<BTreeSet<_>>();
        let initial = if physical == 0 { 13 } else { 12 };
        let history = initial + u32::try_from(new.difference(&old).count()).unwrap();
        after[definition + 67 + physical * 12..definition + 71 + physical * 12]
            .copy_from_slice(&history.to_le_bytes());
    }
    after
}

#[test]
#[ignore = "requires18 independent Binary source views and144 read-only SDK reports"]
fn binary_original_and_literal_after_views_have_exact_rows_keys_aliases_and_preservation() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_BINARY_INDEXED_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute());
    assert_eq!(
        digest(&fs::read(dir.join("manifest.json")).unwrap()),
        "3f850ca34f51174507af991070525e89b8f73ce201c14c438d0117b1dfbbb920"
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
            "insert-null",
            "h5-binary-insert-null.accdb",
            "894790c770abf08e4154347432bf7f3bed690c13461ab175a67573e7ce9fe46a",
        ),
        (
            5_u8,
            "insert-empty",
            "h5-binary-insert-empty.accdb",
            "75fb2c2082b32ff725044fd6b4fa8c8dd837e02b255cc996e8315df449015ab5",
        ),
        (
            5_u8,
            "insert-existing",
            "h5-binary-insert-existing.accdb",
            "de3c70d4f72c71157a250126fd62ed0ca01e9bbe17a2942d837bc75e119e6bf0",
        ),
        (
            5_u8,
            "insert-seven",
            "h5-binary-insert-seven.accdb",
            "52bcbb2258fe64acfbb8be434bde07d0bccd682ae84171f1e7103d021af223ed",
        ),
        (
            5_u8,
            "insert-nine",
            "h5-binary-insert-nine.accdb",
            "c720094bd6cf26d1e69fe9dc84ce09b5d01ca283ca81a189c084af457e9dc531",
        ),
        (
            5_u8,
            "insert-maximum",
            "h5-binary-insert-maximum.accdb",
            "3da6358b4e5e45284eef54d44750180473a9c428569a46f42d46f1e2fc45bf0c",
        ),
        (
            5_u8,
            "batch",
            "h5-binary-batch.accdb",
            "9bc273e1fa042f693e5355b05e7ee16390be27f2ca25813d1e1160ad0d545d76",
        ),
        (
            5_u8,
            "batch-reverse",
            "h5-binary-batch-reverse.accdb",
            "365c324531c49cda303071a39fd3264c0383d436ccbd3061fb0b00f88787da08",
        ),
        (
            6_u8,
            "before",
            "h6-binary-before.accdb",
            "7cd59e5aff849c5ac0567fbd69a46f575956f6bc3cb9071b2e351e8dcaf5d8fd",
        ),
        (
            6_u8,
            "insert-null",
            "h6-binary-insert-null.accdb",
            "1cddf2a16f655874d37ecdd469b591f21e6fa228b4f89bd8e80f739ca733a8ad",
        ),
        (
            6_u8,
            "insert-empty",
            "h6-binary-insert-empty.accdb",
            "7481e9015336dea239802be05c686e2d50d3bb82c2a6e07245633f372d96448b",
        ),
        (
            6_u8,
            "insert-existing",
            "h6-binary-insert-existing.accdb",
            "64d2b14a20e49ba651f5e01ced5a7137affe3c320ac41bf0fe76016be5a8e62b",
        ),
        (
            6_u8,
            "insert-seven",
            "h6-binary-insert-seven.accdb",
            "67c7398d778130532a211c9a02ae5b618925aafccdc5d88f8c0ca00eff81afc3",
        ),
        (
            6_u8,
            "insert-nine",
            "h6-binary-insert-nine.accdb",
            "9a9cc3c078104ec1f09f44357f2138994e93461b27fbb1e13653fb465507cf9a",
        ),
        (
            6_u8,
            "insert-maximum",
            "h6-binary-insert-maximum.accdb",
            "c309714e6085c8557ae2deee14abcf0cb7674d68698a34b2388143e3d94bc684",
        ),
        (
            6_u8,
            "batch",
            "h6-binary-batch.accdb",
            "f9caec0ef0b46260b31fd6c9c0368476924a28033653103530169d7ea6a83141",
        ),
        (
            6_u8,
            "batch-reverse",
            "h6-binary-batch-reverse.accdb",
            "10ed68d96a9c5cbeda35ebccefaff55ebf4184f9f5b081e0af59d6d054f3d46d",
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
        let mut states = original_rows();
        states.extend(requests(recipe));
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
            let original_families = original_rows()
                .iter()
                .map(|row| key(row, physical))
                .collect::<BTreeSet<_>>();
            let families = states
                .iter()
                .map(|row| key(row, physical))
                .collect::<BTreeSet<_>>();
            let initial = if physical == 0 { 13 } else { 12 };
            let history =
                initial + u32::try_from(families.difference(&original_families).count()).unwrap();
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
