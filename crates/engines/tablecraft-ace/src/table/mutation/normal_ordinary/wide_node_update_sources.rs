//! Full independent raw replacement/partition replay, then actual native Source.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_wide_node_update::{self as fixtures, Case};

const SCOPE: CatalogScope = CatalogScope::new(287);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn units(value: &Value) -> i64 {
    match value {
        Value::Int32(value) => i64::from(*value),
        Value::Int64(value) => *value,
        Value::Currency(value) => value.scaled(),
        _ => panic!("outside frozen present scalar cells"),
    }
}

fn key(value: &Value, row: u32, descending: bool) -> Vec<u8> {
    let mut bytes = vec![if descending { 0x80 } else { 0x7f }];
    let value = ((units(value) as u64) ^ 0x8000000000000000).to_be_bytes();
    bytes.extend(
        value
            .into_iter()
            .map(|byte| if descending { !byte } else { byte }),
    );
    bytes.extend_from_slice(&row.to_be_bytes());
    bytes
}

fn write_page(expected: &mut [u8], original: &[u8], page: u32, entries: &[Vec<u8>]) {
    let base = page as usize * 4096;
    assert_eq!(&expected[base..base + 4096], &original[base..base + 4096]);
    assert_eq!(&original[base + 24..base + 26], &[0, 0]);
    let mut active = Vec::new();
    let mut bitmap = [0; 453];
    for entry in entries {
        active.extend_from_slice(entry);
        bitmap[active.len() / 8] |= 1 << (active.len() % 8);
    }
    assert!(active.len() <= 3616);
    expected[base + 2..base + 4].copy_from_slice(&((3616 - active.len()) as u16).to_le_bytes());
    expected[base + 27..base + 480].copy_from_slice(&bitmap);
    expected[base + 480..base + 480 + active.len()].copy_from_slice(&active);
}

fn reconstructed(case: &Case) -> (Vec<u8>, Vec<Vec<Value>>) {
    let original = fixtures::original_rows(case);
    let mut rows = original.clone();
    let mut expected = case.before.clone();
    let mut changed = Vec::new();
    let mut key_changed = false;
    for (column, assignments) in fixtures::groups(case) {
        let field = match column {
            "KeyValue" => 1,
            "Marker" => 2,
            _ => unreachable!(),
        };
        for assignment in assignments {
            assert_eq!(original[assignment.row][field], assignment.expected);
            if assignment.expected == assignment.replacement {
                continue;
            }
            let fact = case.facts.records[assignment.row];
            let (offset, bytes) = if field == 1 {
                key_changed = true;
                (
                    fact.offset + 6,
                    units(&assignment.replacement).to_le_bytes().to_vec(),
                )
            } else {
                (
                    fact.offset + 14,
                    (units(&assignment.replacement) as i32)
                        .to_le_bytes()
                        .to_vec(),
                )
            };
            expected[offset..offset + bytes.len()].copy_from_slice(&bytes);
            rows[assignment.row][field] = fixtures::cell(&assignment.replacement);
            changed.push(assignment.row);
        }
    }
    changed.sort_unstable();
    changed.dedup();
    assert_eq!(changed.len(), fixtures::changed_rows(case.recipe));
    if key_changed {
        for physical in 1..3 {
            let mut entries = rows
                .iter()
                .zip(case.facts.records)
                .map(|(row, fact)| key(&row[1], fact.reference, physical == 2))
                .collect::<Vec<_>>();
            entries.sort();
            let [left, tail] = case.facts.children[physical - 1];
            assert_eq!(entries.len(), 400);
            write_page(&mut expected, &case.before, left, &entries[..200]);
            write_page(&mut expected, &case.before, tail, &entries[200..]);
            let mut fence = entries[199].clone();
            fence.extend_from_slice(&left.to_be_bytes());
            write_page(
                &mut expected,
                &case.before,
                case.facts.roots[physical],
                &[fence],
            );
        }
    }
    assert_eq!(expected, case.after);
    (expected, rows)
}

fn qualify(case: &Case, raw: &[u8], expected: &[Vec<Value>]) {
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
    assert_eq!(source.table.rows(), expected);
    assert_eq!(source.table.schema().declared_rows(), 400);
    assert_eq!(
        source.table.schema().definition_page(),
        case.facts.definition
    );
    assert_eq!(source.records.len(), 400);
    for (record, fact) in source.records.iter().zip(case.facts.records) {
        assert_eq!(record.reference, fact.reference);
        assert_eq!(record.offset, fact.offset);
        assert_eq!(record.data.len(), 19);
        assert_eq!(&record.data[..2], &[3, 0]);
        assert_eq!(record.data[18], 7);
        assert_eq!(record.data, raw[fact.offset..fact.offset + 19]);
    }
    let checked = crate::table::indexes::check_table_retained(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 400);
    assert_eq!(checked.indexes().len(), 4);
    for (alias, physical) in [
        ("PrimaryID", 0),
        ("Ascending", 1),
        ("Descending", 2),
        ("Alias", 1),
    ] {
        let index = checked
            .indexes()
            .iter()
            .find(|selected| selected.index().name() == alias)
            .unwrap();
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().root_page(), case.facts.roots[physical]);
        assert_eq!(index.index().data_number(), physical as u32);
        let counter = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            case.table,
            alias,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.value(), if physical == 0 { 400 } else { 1 });
        let field = usize::from(physical != 0);
        let mut order = (0..400).collect::<Vec<_>>();
        order.sort_by(|left, right| {
            let comparison = units(&expected[*left][field]).cmp(&units(&expected[*right][field]));
            let comparison = if physical == 2 {
                comparison.reverse()
            } else {
                comparison
            };
            comparison.then(
                case.facts.records[*left]
                    .reference
                    .cmp(&case.facts.records[*right].reference),
            )
        });
        let read = read_table_index(
            &mut image.reader(),
            SCOPE,
            case.table,
            alias,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            read.rows(),
            order
                .into_iter()
                .map(|ordinal| expected[ordinal].clone())
                .collect::<Vec<_>>()
        );
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 400);
    assert!(constraints.relationships().is_empty());
}

#[test]
#[ignore = "requires all 20 independent stable-count numeric NODE source images"]
fn wider_node_update_sources_require_full_independent_replay_and_complete_current_source() {
    let cases = fixtures::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    assert_eq!(cases.len(), 20);
    for case in cases {
        let (after, rows) = reconstructed(&case);
        qualify(&case, &case.before, &fixtures::original_rows(&case));
        qualify(&case, &after, &rows);
    }
}
