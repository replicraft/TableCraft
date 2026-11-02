//! Independent literal fixed cells/key pages plus actual native Source; no planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_wide_update::{self as fixtures, Case};

const SCOPE: CatalogScope = CatalogScope::new(283);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn units(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Int32(value) => Some(i64::from(*value)),
        Value::Int64(value) => Some(*value),
        Value::Currency(value) => Some(value.scaled()),
        _ => panic!("outside frozen native carriers"),
    }
}

fn column(name: &str) -> usize {
    match name {
        "Id" => 0,
        "KeyValue" => 1,
        "Marker" => 2,
        _ => panic!("outside original column dictionary"),
    }
}

fn wide(value: Option<i64>, money: bool) -> Value {
    match value {
        None => Value::Null,
        Some(value) if money => Value::Currency(Currency::from_scaled(value)),
        Some(value) => Value::Int64(value),
    }
}

fn original_rows(case: &Case) -> Vec<Vec<Value>> {
    let primary = case.table.ends_with("WidePrimary");
    let money = case.table.starts_with("Currency");
    [Some(i64::MIN), None, Some(i64::MAX)]
        .into_iter()
        .enumerate()
        .map(|(ordinal, key)| {
            let id = if primary {
                wide(Some([i64::MIN, 0, i64::MAX][ordinal]), money)
            } else {
                Value::Int32(ordinal as i32 + 1)
            };
            vec![id, wide(key, money), Value::Int32([17, 34, 51][ordinal])]
        })
        .collect()
}

fn wire(value: &Value, reference: u32, descending: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(value_units) = units(value) {
        bytes.push(if descending { 0x80 } else { 0x7f });
        let payload = if matches!(value, Value::Int32(_)) {
            ((value_units as u32) ^ 0x80000000).to_be_bytes().to_vec()
        } else {
            ((value_units as u64) ^ 0x8000000000000000)
                .to_be_bytes()
                .to_vec()
        };
        bytes.extend(
            payload
                .into_iter()
                .map(|byte| if descending { !byte } else { byte }),
        );
    } else {
        bytes.push(if descending { 0xff } else { 0 });
    }
    bytes.extend_from_slice(&reference.to_be_bytes());
    bytes
}

fn page(active_rows: &[Vec<Value>], case: &Case, physical: usize) -> (Vec<u8>, [u8; 453]) {
    let key_column = usize::from(physical != 0);
    let mut entries = active_rows
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            wire(
                &row[key_column],
                case.layout.data << 8 | ordinal as u32,
                physical == 2,
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    let mut active = Vec::new();
    let mut mask = [0; 453];
    for entry in entries {
        active.extend_from_slice(&entry);
        mask[active.len() / 8] |= 1 << (active.len() % 8);
    }
    (active, mask)
}

fn reconstructed(case: &Case) -> (Vec<u8>, Vec<Vec<Value>>) {
    let old_rows = original_rows(case);
    let mut rows = old_rows.clone();
    let mut expected = case.before.clone();
    let primary = case.table.ends_with("WidePrimary");
    let offsets = if primary { [0, 8, 16] } else { [0, 4, 12] };
    let data = case.layout.data as usize * 4096;
    let mut changed = 0_u8;
    assert_eq!(
        u16::from_le_bytes(case.before[data + 12..data + 14].try_into().unwrap()),
        3
    );
    for (name, assignments) in &case.groups {
        let field = column(name);
        for assignment in assignments {
            assert_eq!(old_rows[assignment.row][field], assignment.expected);
            if assignment.expected != assignment.replacement {
                changed |= 1 << assignment.row;
            }
            let start = 4096 - case.layout.record_length * (assignment.row + 1);
            assert_eq!(
                u16::from_le_bytes(
                    case.before[data + 14 + assignment.row * 2..data + 16 + assignment.row * 2]
                        .try_into()
                        .unwrap()
                ) as usize,
                start
            );
            assert_eq!(&case.before[data + start..data + start + 2], &[3, 0]);
            let mask = data + start + case.layout.record_length - 1;
            if assignment.replacement == Value::Null {
                expected[mask] &= !(1 << field);
            } else {
                let bytes = match &assignment.replacement {
                    Value::Int32(value) => value.to_le_bytes().to_vec(),
                    Value::Int64(value) => value.to_le_bytes().to_vec(),
                    Value::Currency(value) => value.scaled().to_le_bytes().to_vec(),
                    _ => panic!("frozen fixed scalar replacement"),
                };
                let offset = data + start + 2 + offsets[field];
                expected[offset..offset + bytes.len()].copy_from_slice(&bytes);
                expected[mask] |= 1 << field;
            }
            rows[assignment.row][field] = fixtures::cell(&assignment.replacement);
        }
    }
    assert_eq!(changed.count_ones() as usize, case.changed);
    for (physical, root) in case.layout.roots.iter().enumerate() {
        let base = *root as usize * 4096;
        let (old_active, old_mask) = page(&old_rows, case, physical);
        assert_eq!(case.before[base], 4);
        assert_eq!(&case.before[base + 8..base + 27], &[0; 19]);
        assert_eq!(&case.before[base + 27..base + 480], &old_mask);
        assert_eq!(
            &case.before[base + 480..base + 480 + old_active.len()],
            old_active
        );
        let (active, mask) = page(&rows, case, physical);
        expected[base + 27..base + 480].copy_from_slice(&mask);
        expected[base + 480..base + 480 + active.len()].copy_from_slice(&active);
        expected[base + 2..base + 4].copy_from_slice(&((3616 - active.len()) as u16).to_le_bytes());
    }
    assert_eq!(expected, case.after);
    (expected, rows)
}

fn qualify(raw: &[u8], rows: &[Vec<Value>], case: &Case) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let catalog = crate::inspect_table_schema(
        &mut image.reader(),
        SCOPE,
        "MSysObjects",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(catalog.columns().len(), 17);
    let source = super::capture(&image, case.table, &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), rows);
    assert_eq!(source.table.schema().declared_rows(), 3);
    assert_eq!(
        source.table.schema().definition_page(),
        case.layout.definition
    );
    assert_eq!(source.records.len(), 3);
    for (ordinal, record) in source.records.iter().enumerate() {
        let start = 4096 - case.layout.record_length * (ordinal + 1);
        assert_eq!(record.reference, case.layout.data << 8 | ordinal as u32);
        assert_eq!(record.offset, case.layout.data as usize * 4096 + start);
        assert_eq!(
            record.data,
            raw[record.offset..record.offset + case.layout.record_length]
        );
    }
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 6);
    for (physical, aliases) in [
        ["PrimaryID", "PrimaryAlias"],
        ["Ascending", "AscendingAlias"],
        ["Descending", "DescendingAlias"],
    ]
    .into_iter()
    .enumerate()
    {
        let key_column = usize::from(physical != 0);
        let mut order = [0, 1, 2];
        order.sort_by(|left, right| {
            let compared = units(&rows[*left][key_column]).cmp(&units(&rows[*right][key_column]));
            let compared = if physical == 2 {
                compared.reverse()
            } else {
                compared
            };
            compared.then(left.cmp(right))
        });
        let expected_rows = order
            .into_iter()
            .map(|ordinal| rows[ordinal].clone())
            .collect::<Vec<_>>();
        for alias in aliases {
            let entry = checked
                .indexes()
                .iter()
                .find(|entry| entry.index().name() == alias)
                .unwrap();
            assert_eq!(entry.status(), IndexCheckStatus::Checked);
            assert_eq!(entry.index().root_page(), case.layout.roots[physical]);
            assert_eq!(entry.index().data_number(), physical as u32);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                case.table,
                alias,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value(), 3);
            let read = read_table_index(
                &mut image.reader(),
                SCOPE,
                case.table,
                alias,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(read.rows(), expected_rows);
        }
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 3);
    assert!(constraints.relationships().is_empty());
}

#[test]
#[ignore = "requires 48 independently qualified wider UPDATE image views"]
fn wider_indexed_update_sources_require_complete_raw_replay_native_source_and_all_aliases() {
    let cases = fixtures::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    assert_eq!(cases.len(), 48);
    for case in cases {
        let (after, rows) = reconstructed(&case);
        qualify(&case.before, &original_rows(&case), &case);
        qualify(&after, &rows, &case);
    }
}
