//! Original/request-derived DATA and key overlays, read-only native qualification.
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

const SCOPE: CatalogScope = CatalogScope::new(196);
const ALIASES: [&str; 6] = [
    "PrimaryID",
    "PrimaryAlias",
    "Ascending",
    "AscendingAlias",
    "Descending",
    "DescendingAlias",
];
type Row = (usize, i32, Option<i64>, i32);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn rows(family: &str, recipe: &str) -> Vec<Row> {
    let mut rows = match family {
        "empty" => Vec::new(),
        "all-null" => vec![(0, 1, None, 17), (1, 2, None, 34), (2, 3, None, 51)],
        "sparse" | "original" => vec![
            (0, 1, Some(i64::MIN), 17),
            (1, 2, None, 34),
            (2, 3, Some(i64::MAX), 51),
        ],
        _ => panic!("unknown source family"),
    };
    match recipe {
        "before" => {}
        "insert-null" => rows.push((3, 4, None, 68)),
        "insert-zero" => rows.push((3, 4, Some(0), 68)),
        "delete-all" => rows.clear(),
        _ => panic!("unknown original request"),
    }
    rows
}

fn values(table: &str, row: Row) -> Vec<Value> {
    let key = row
        .2
        .map(|key| {
            if table == "CurrencyKeys" {
                Value::Currency(Currency::from_scaled(key))
            } else {
                Value::Int64(key)
            }
        })
        .unwrap_or(Value::Null);
    vec![Value::Int32(row.1), key, Value::Int32(row.3)]
}

fn history(family: &str, recipe: &str, physical: usize) -> u32 {
    if !recipe.starts_with("insert-") {
        return 3;
    }
    if physical == 0 || family == "empty" || recipe == "insert-zero" {
        4
    } else {
        3
    }
}

fn key(row: Row, physical: usize) -> Option<i64> {
    if physical == 0 {
        Some(i64::from(row.1))
    } else {
        row.2
    }
}

fn order(rows: &mut [Row], physical: usize) {
    rows.sort_by(|left, right| {
        let comparison = key(*left, physical).cmp(&key(*right, physical));
        let comparison = if physical == 2 {
            comparison.reverse()
        } else {
            comparison
        };
        if comparison == Ordering::Equal {
            left.0.cmp(&right.0)
        } else {
            comparison
        }
    });
}

fn wire(row: Row, data: u32, physical: usize) -> Vec<u8> {
    let mut result = Vec::new();
    match key(row, physical) {
        None => result.push(if physical == 2 { 255 } else { 0 }),
        Some(value) => {
            result.push(if physical == 2 { 128 } else { 127 });
            let mut payload = if physical == 0 {
                ((value as i32 as u32) ^ 0x8000_0000).to_be_bytes().to_vec()
            } else {
                ((value as u64) ^ 0x8000_0000_0000_0000)
                    .to_be_bytes()
                    .to_vec()
            };
            if physical == 2 {
                for byte in payload.iter_mut() {
                    *byte = !*byte;
                }
            }
            result.extend_from_slice(&payload);
        }
    }
    result.extend_from_slice(&((data << 8) | row.0 as u32).to_be_bytes());
    result
}

fn leaves(before: &[u8], after: &mut [u8], data: u32, roots: [u32; 3], states: &[Row]) {
    for (physical, root) in roots.iter().enumerate() {
        let base = *root as usize * 4096;
        let mut ordered = states.to_vec();
        order(&mut ordered, physical);
        let mut bitmap = [0_u8; 453];
        let mut active = Vec::new();
        for row in ordered {
            active.extend_from_slice(&wire(row, data, physical));
            bitmap[active.len() / 8] |= 1 << (active.len() % 8);
        }
        after[base + 27..base + 480].copy_from_slice(&bitmap);
        after[base + 480..base + 480 + active.len()].copy_from_slice(&active);
        after[base + 2..base + 4].copy_from_slice(&((3616 - active.len()) as u16).to_le_bytes());
        assert_eq!(
            &after[base + 480 + active.len()..base + 4096],
            &before[base + 480 + active.len()..base + 4096]
        );
    }
}

fn literal_before(
    original: &[u8],
    definition: u32,
    data: u32,
    roots: [u32; 3],
    family: &str,
) -> Vec<u8> {
    let mut source = original.to_vec();
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    match family {
        "empty" => {
            source[td + 16..td + 20].copy_from_slice(&0_u32.to_le_bytes());
            for slot in 0..3 {
                source[page + 14 + slot * 2..page + 16 + slot * 2]
                    .copy_from_slice(&((4077_u16 - slot as u16 * 19) | 0xc000).to_le_bytes());
            }
            leaves(original, &mut source, data, roots, &[]);
        }
        "all-null" => {
            for slot in [0, 2] {
                source[page + 4077 - 19 * slot + 18] = 5;
            }
            leaves(original, &mut source, data, roots, &rows(family, "before"));
        }
        "sparse" => {
            source[td + 41..td + 43].copy_from_slice(&5_u16.to_le_bytes());
            for (position, native) in [(1, 3_u16), (2, 4_u16)] {
                let at = td + 99 + 25 * position;
                source[at + 5..at + 7].copy_from_slice(&native.to_le_bytes());
                source[at + 9..at + 11].copy_from_slice(&native.to_le_bytes());
            }
            for physical in [1, 2] {
                let at = td + 212 + 52 * physical + 4;
                source[at..at + 2].copy_from_slice(&3_u16.to_le_bytes());
            }
            for slot in 0..3 {
                let start = page + 4077 - slot * 19;
                source[start..start + 2].copy_from_slice(&5_u16.to_le_bytes());
                source[start + 18] = if slot == 1 { 0x11 } else { 0x19 };
            }
        }
        _ => panic!("unknown derivative family"),
    }
    source
}

fn literal_after(
    source: &[u8],
    definition: u32,
    data: u32,
    roots: [u32; 3],
    family: &str,
    recipe: &str,
) -> Vec<u8> {
    let mut after = source.to_vec();
    if recipe == "before" {
        return after;
    }
    let td = definition as usize * 4096;
    let page = data as usize * 4096;
    let states = rows(family, recipe);
    if recipe.starts_with("insert-") {
        let mut record = Vec::from(if family == "sparse" { [5, 0] } else { [3, 0] });
        record.extend_from_slice(&4_i32.to_le_bytes());
        record.extend_from_slice(&0_i64.to_le_bytes());
        record.extend_from_slice(&68_i32.to_le_bytes());
        record.push(match (family, recipe) {
            ("sparse", "insert-null") => 0x11,
            ("sparse", _) => 0x19,
            (_, "insert-null") => 5,
            _ => 7,
        });
        after[page + 4020..page + 4039].copy_from_slice(&record);
        after[page + 20..page + 22].copy_from_slice(&4020_u16.to_le_bytes());
        after[page + 12..page + 14].copy_from_slice(&4_u16.to_le_bytes());
        after[page + 2..page + 4].copy_from_slice(&3998_u16.to_le_bytes());
    } else {
        assert_eq!(recipe, "delete-all");
        for slot in 0..3 {
            after[page + 14 + slot * 2..page + 16 + slot * 2]
                .copy_from_slice(&((4077_u16 - slot as u16 * 19) | 0xc000).to_le_bytes());
        }
    }
    after[td + 16..td + 20].copy_from_slice(&(states.len() as u32).to_le_bytes());
    leaves(source, &mut after, data, roots, &states);
    for physical in 0..3 {
        after[td + 67 + 12 * physical..td + 71 + 12 * physical]
            .copy_from_slice(&history(family, recipe, physical).to_le_bytes());
    }
    after
}

fn read_view(raw: &[u8], header: u8, selected: &str, family: &str, recipe: &str) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    for table in ["Int64Keys", "CurrencyKeys"] {
        let definition = match (header, table) {
            (5, "Int64Keys") => 89,
            (5, _) => 96,
            (6, "Int64Keys") => 106,
            (6, _) => 113,
            _ => unreachable!(),
        };
        let sparse = table == selected && family == "sparse";
        let active = if table == selected {
            rows(family, recipe)
        } else {
            rows("original", "before")
        };
        let expected = active
            .iter()
            .map(|row| values(table, *row))
            .collect::<Vec<_>>();
        let snapshot =
            read_table(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap();
        assert_eq!(snapshot.rows(), expected);
        assert_eq!(snapshot.schema().definition_page(), definition);
        for (position, field) in snapshot.schema().columns().iter().enumerate() {
            assert_eq!(
                field.number(),
                if sparse {
                    [0, 3, 4][position]
                } else {
                    position
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
            assert_eq!(index.root_page(), definition + 2 + physical as u32);
            assert_eq!(
                index.columns()[0].number(),
                if physical == 0 {
                    0
                } else if sparse {
                    3
                } else {
                    1
                }
            );
            let stored = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                table,
                index.name(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                stored.value(),
                if table == selected {
                    history(family, recipe, physical)
                } else {
                    3
                }
            );
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
        if sparse {
            let refused =
                super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                    .err()
                    .unwrap();
            assert_eq!(refused, EditError::Unsupported(WriteBlocker::ActiveContent));
        } else {
            super::super::normal_ordinary::capture(&image, table, &mut budget(), &|| false)
                .unwrap();
            image
                .check_native_ordinary_fixed_table(table, &mut budget(), &|| false)
                .unwrap();
        }
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires44 independently native/SDK-readable numeric follow-on views"]
fn numeric_followons_separate_sparse_read_support_from_current_dense_source_admission() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR").unwrap());
    let old = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_INDEXED_SOURCE_DIR").unwrap());
    assert!(dir.is_absolute() && old.is_absolute());
    assert_eq!(
        digest(&fs::read(dir.join("manifest.json")).unwrap()),
        "5b6364e5546cec155a3a886d414d1fe7061abb1ec3607a5cfba1fcd84c810725"
    );
    let originals = [
        fs::read(old.join("h5-integral-original.accdb")).unwrap(),
        fs::read(old.join("h6-integral-original.accdb")).unwrap(),
    ];
    assert_eq!(
        digest(&originals[0]),
        "e7538a1a69ec23953fa7f00727c277617d85848a047ccd0a2b759e618c87e5a1"
    );
    assert_eq!(
        digest(&originals[1]),
        "3779da0b09d0fd424e62929149578f63309b6d8f63a0ef7502e658b20ced377e"
    );
    for (header, table, family, recipe, filename, hash) in [
        (
            5_u8,
            "Int64Keys",
            "empty",
            "before",
            "h5-Int64Keys-empty-before.accdb",
            "c73a65b461f0d52a73ec47aeba77e40ecf1d2c6686744360d4ca977049690dc8",
        ),
        (
            5_u8,
            "Int64Keys",
            "empty",
            "insert-null",
            "h5-Int64Keys-empty-insert-null.accdb",
            "790535e87bbc1823ce0424673911be29c6d3c8873d19553672da391c980b375a",
        ),
        (
            5_u8,
            "Int64Keys",
            "empty",
            "insert-zero",
            "h5-Int64Keys-empty-insert-zero.accdb",
            "c1e17f6ac9665e9e461537b4704bc8af9ac1dd6dfc3bcceeebcc786bd518d17e",
        ),
        (
            5_u8,
            "Int64Keys",
            "all-null",
            "before",
            "h5-Int64Keys-all-null-before.accdb",
            "63f2a5233dd6c0d4ec77472f57174264e6536193ab90265b335a84bfb1a73162",
        ),
        (
            5_u8,
            "Int64Keys",
            "all-null",
            "insert-null",
            "h5-Int64Keys-all-null-insert-null.accdb",
            "6b280b1f45195e6d4444a72c6a4ed31ee71e0623a862eae55165456aab0cb4cd",
        ),
        (
            5_u8,
            "Int64Keys",
            "all-null",
            "insert-zero",
            "h5-Int64Keys-all-null-insert-zero.accdb",
            "1db6655504e733a1322178f0c1448634643e83b939d541f84d026c88101d4632",
        ),
        (
            5_u8,
            "Int64Keys",
            "all-null",
            "delete-all",
            "h5-Int64Keys-all-null-delete-all.accdb",
            "8e92fda8b4b35d22399c7d705d969c5e80594fcf8d897b1830712036f2526cbd",
        ),
        (
            5_u8,
            "Int64Keys",
            "sparse",
            "before",
            "h5-Int64Keys-sparse-before.accdb",
            "b5bbc821a49442bcbf876ea5fcb8c560b1825a2440f0e2c5d54c563e524ab89a",
        ),
        (
            5_u8,
            "Int64Keys",
            "sparse",
            "insert-null",
            "h5-Int64Keys-sparse-insert-null.accdb",
            "a492018abad9b265e840c8871ab6903da0eb8ba1f97c6f9a7e73df414795b0eb",
        ),
        (
            5_u8,
            "Int64Keys",
            "sparse",
            "insert-zero",
            "h5-Int64Keys-sparse-insert-zero.accdb",
            "bd98e35d1b2e7b9291613d970bc6fe4d3690231bfc58925ab0216ee6fdb24d94",
        ),
        (
            5_u8,
            "Int64Keys",
            "sparse",
            "delete-all",
            "h5-Int64Keys-sparse-delete-all.accdb",
            "aec4044e7c1e4fb7d0954f3ce6a8d54853b8423f48743c7601ffd8d8013f009e",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "empty",
            "before",
            "h5-CurrencyKeys-empty-before.accdb",
            "6dbfe1d18755164a42b0b6567d13cae9e2183cd51ff12b65614b98071a83ccf3",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "empty",
            "insert-null",
            "h5-CurrencyKeys-empty-insert-null.accdb",
            "cbc31666db762f062c305f9fae1e9306bfadd90e74d41a57a65a9cad456c84c1",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "empty",
            "insert-zero",
            "h5-CurrencyKeys-empty-insert-zero.accdb",
            "28ca3c43882d8b3650b89d8eef1b090ca5d3d661a86d37b6c3d7053eb58f2380",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "all-null",
            "before",
            "h5-CurrencyKeys-all-null-before.accdb",
            "2b1b112c32e211b570f39a5ed7eb479e7fb4fea013c79ba0e51863480bdf8c0f",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "all-null",
            "insert-null",
            "h5-CurrencyKeys-all-null-insert-null.accdb",
            "ec164c3c7d8f293f535169b9da6cfe776ae40553952c9fe4ac891203fd68519f",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "all-null",
            "insert-zero",
            "h5-CurrencyKeys-all-null-insert-zero.accdb",
            "c29178aba34d6068b02acd728e1e336b26cf2aa6954199b9d42f5cbffe0f8841",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "all-null",
            "delete-all",
            "h5-CurrencyKeys-all-null-delete-all.accdb",
            "40e423e197bdf0976387b89420ee1052c05a66b73892c4cf356dbe0bb4d31317",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "sparse",
            "before",
            "h5-CurrencyKeys-sparse-before.accdb",
            "aa8b5d16838dab85df89b42fa61deb1dec0128e6b1408b2236e3d0cb2f93aa40",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "sparse",
            "insert-null",
            "h5-CurrencyKeys-sparse-insert-null.accdb",
            "5828142823e3af02372f2e99cfd9bc23922588e6428edefb6e1d717cd714c3ff",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "sparse",
            "insert-zero",
            "h5-CurrencyKeys-sparse-insert-zero.accdb",
            "4ead59aa08f76ce7a70ef194e1619f3711682875e4c6161301a22547e85170c1",
        ),
        (
            5_u8,
            "CurrencyKeys",
            "sparse",
            "delete-all",
            "h5-CurrencyKeys-sparse-delete-all.accdb",
            "61d06206dcae924bee4dcd7ebd54ae10f122b223f3dde28802a34de96866857a",
        ),
        (
            6_u8,
            "Int64Keys",
            "empty",
            "before",
            "h6-Int64Keys-empty-before.accdb",
            "97ec82868ed17533051bb0345864b642a44951d45b41fcd9ffffc61437589a0b",
        ),
        (
            6_u8,
            "Int64Keys",
            "empty",
            "insert-null",
            "h6-Int64Keys-empty-insert-null.accdb",
            "d9b34465d75eda6ac4dfadae0f5f719e95b11d41879690e176f10ef6bcee78ba",
        ),
        (
            6_u8,
            "Int64Keys",
            "empty",
            "insert-zero",
            "h6-Int64Keys-empty-insert-zero.accdb",
            "4996b9f0df9201972e2867d02e97ca6abb4fd9be3d8c3864d7e61b58fa61972e",
        ),
        (
            6_u8,
            "Int64Keys",
            "all-null",
            "before",
            "h6-Int64Keys-all-null-before.accdb",
            "c09cd358566950b85c3b7cccb207d4b54831551f37aa43cbc6a7841d57333bef",
        ),
        (
            6_u8,
            "Int64Keys",
            "all-null",
            "insert-null",
            "h6-Int64Keys-all-null-insert-null.accdb",
            "994ebcd2c00ce0e81d870f5db386e56d63e872741ef0b8690d696006d9cf71f2",
        ),
        (
            6_u8,
            "Int64Keys",
            "all-null",
            "insert-zero",
            "h6-Int64Keys-all-null-insert-zero.accdb",
            "969f0b7ab6e132287215ee819e09bd6c4d7e53fa39312fd6ab479d94c4a2694e",
        ),
        (
            6_u8,
            "Int64Keys",
            "all-null",
            "delete-all",
            "h6-Int64Keys-all-null-delete-all.accdb",
            "bc64bd53f6aab1cf12c3b5754221f065567df7dce4a0ecd702c54b0af9580087",
        ),
        (
            6_u8,
            "Int64Keys",
            "sparse",
            "before",
            "h6-Int64Keys-sparse-before.accdb",
            "24d227706b7dd46a52c96b6790f24c716eb75075ffa99c7d82b239f6691f389f",
        ),
        (
            6_u8,
            "Int64Keys",
            "sparse",
            "insert-null",
            "h6-Int64Keys-sparse-insert-null.accdb",
            "b7d775411cc19ca9ddb216dae4d85cd7d37c6c70643bd3e4004048ebd4a60965",
        ),
        (
            6_u8,
            "Int64Keys",
            "sparse",
            "insert-zero",
            "h6-Int64Keys-sparse-insert-zero.accdb",
            "cf87a6c34059dc212a9785e72ed0f7938fa3a9f3a15864eea22c6fb064275faf",
        ),
        (
            6_u8,
            "Int64Keys",
            "sparse",
            "delete-all",
            "h6-Int64Keys-sparse-delete-all.accdb",
            "4af73abe48d49d9d3df2c5e9ada345783a5dbacbc000d43853dad39865d3f04e",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "empty",
            "before",
            "h6-CurrencyKeys-empty-before.accdb",
            "9600a6e1449f28485af22006de9730f9c4ff013d712a0606c38715fc1ac9a35a",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "empty",
            "insert-null",
            "h6-CurrencyKeys-empty-insert-null.accdb",
            "4a65d29c66d4c63921006fc2051a9fe5cf6a88203b8e94215da85bbd888356e5",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "empty",
            "insert-zero",
            "h6-CurrencyKeys-empty-insert-zero.accdb",
            "6703ed09a5d6ee3ba5c578e0741873fe4dc0555c8a66f43b192b4e40a936bb28",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "all-null",
            "before",
            "h6-CurrencyKeys-all-null-before.accdb",
            "3b22521e8ee606bd5e3fe6203500655af57e8dda73a4424825f74cfbd09705f2",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "all-null",
            "insert-null",
            "h6-CurrencyKeys-all-null-insert-null.accdb",
            "ef60d39985748d91a9f1b0288016b906c6cc698000ff3fad5fda3b276efff6a3",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "all-null",
            "insert-zero",
            "h6-CurrencyKeys-all-null-insert-zero.accdb",
            "9d61f894a55d71c3cbb59ee850ef049b893a1fa73cca13644d62fa16739bb7a9",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "all-null",
            "delete-all",
            "h6-CurrencyKeys-all-null-delete-all.accdb",
            "64e0b24ed4c428598b9deba225179831232f33f90a513b5fc5645ee6493fac79",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "sparse",
            "before",
            "h6-CurrencyKeys-sparse-before.accdb",
            "f2bca089d76ccf2b4c30510f886be9a84ef76df619fa106827b9ebbcf38e5c89",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "sparse",
            "insert-null",
            "h6-CurrencyKeys-sparse-insert-null.accdb",
            "f14349af5286d078307dd9270cb2335b9291abf544fb4c6fb44e715db072e8c6",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "sparse",
            "insert-zero",
            "h6-CurrencyKeys-sparse-insert-zero.accdb",
            "c5afd1793c08366e9dbe445ba493731d752b19d93cd8cdd3baea7b61986cc986",
        ),
        (
            6_u8,
            "CurrencyKeys",
            "sparse",
            "delete-all",
            "h6-CurrencyKeys-sparse-delete-all.accdb",
            "0bdb05798fcb77c6468a864aa371a17e5384d49026556ae7d6ce620d7f90fd56",
        ),
    ] {
        let raw = fs::read(dir.join(filename)).unwrap();
        assert_eq!(digest(&raw), hash);
        let original = &originals[usize::from(header - 5)];
        let definition = match (header, table) {
            (5, "Int64Keys") => 89,
            (5, _) => 96,
            (6, "Int64Keys") => 106,
            (6, _) => 113,
            _ => unreachable!(),
        };
        let data = definition + 5;
        let roots = [definition + 2, definition + 3, definition + 4];
        let before = literal_before(original, definition, data, roots, family);
        let expected = literal_after(&before, definition, data, roots, family, recipe);
        assert_eq!(
            raw, expected,
            "complete original-only byte complement {table}/{family}/{recipe}"
        );
        read_view(&raw, header, table, family, recipe);
    }
}
