//! Source/whole-expectation gates for simultaneous physical owners in two tables.
pub(in crate::table) mod model;
mod preview;

use crate::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
    read_table,
};
use model::{Case, Mode, PAGE, Table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn qualify_table(bytes: &[u8], table: &Table, expected: bool, histories: [u32; 2]) {
    let image = capture(bytes);
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        table.name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 500);
    assert_eq!(checked.indexes().len(), 4);
    for (position, index) in checked.indexes().iter().enumerate() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().data_number(), (position / 2) as u32);
        assert!(!index.index().constraints().unwrap().primary_key());
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        table.name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 500);
    assert_eq!(constraints.fields().len(), 3);
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.relationships().is_empty());
    let snapshot = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        table.name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.schema().definition_page(), table.definition);
    let rows = if expected {
        &table.final_rows
    } else {
        &table.original
    };
    assert_eq!(snapshot.rows().len(), rows.len());
    for (actual, row) in snapshot.rows().iter().zip(rows) {
        assert_eq!(
            actual,
            &vec![
                Value::Int32(row.id),
                row.left.map_or(Value::Null, Value::Int32),
                row.right.map_or(Value::Null, Value::Int32)
            ]
        );
    }
    for (physical, history) in histories.into_iter().enumerate() {
        let offset = table.definition as usize * PAGE + 67 + physical * 12;
        assert_eq!(
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            history
        );
    }
}

fn free(bytes: &[u8], page: u32) -> bool {
    let range = model::span(bytes, 1 << 8);
    assert_eq!(bytes[range.start], 0);
    assert_eq!(&bytes[range.start + 1..range.start + 5], &[0; 4]);
    bytes[range.start + 5 + page as usize / 8] & (1 << (page % 8)) != 0
}

fn qualify_case(case: &Case) {
    assert_eq!(case.tables[0].definition, 10);
    assert_eq!(case.tables[1].definition, 20);
    assert!(case.tables[1].name < case.tables[0].name);
    assert_eq!(case.before.len(), case.eof as usize * PAGE);
    assert_eq!(
        case.expected.len(),
        case.before.len() + case.added.len() * PAGE
    );
    let mut next = case.eof;
    for table in &case.tables {
        qualify_table(&case.before, table, false, [700, 800]);
        qualify_table(&case.expected, table, true, table.history);
        for page in &table.data {
            let base = *page as usize * PAGE;
            let count = usize::from(u16::from_le_bytes(
                case.before[base + 12..base + 14].try_into().unwrap(),
            ));
            assert_eq!(
                &case.before[base..base + 14 + count * 2],
                &case.expected[base..base + 14 + count * 2]
            );
            assert!(!free(&case.before, *page));
        }
        let map = (table.definition + 1) as usize * PAGE;
        assert_eq!(
            &case.before[map..map + PAGE],
            &case.expected[map..map + PAGE]
        );
        if let Some(target) = table.target {
            let root = model::span(&case.before, table.original[0].reference);
            assert_eq!(&case.before[root.clone()], &case.expected[root]);
            assert_eq!(table.original[0].reference, ((table.definition + 2) << 8));
            assert_eq!((target >> 8), if table.definition == 10 { 30 } else { 31 });
        }
        for physical in 0..2 {
            let matching = case
                .added
                .iter()
                .filter(|item| item.definition == table.definition && item.physical == physical)
                .collect::<Vec<_>>();
            if table.fitted {
                assert!(matching.is_empty());
            } else {
                assert_eq!(matching.len(), 2);
                assert_eq!(matching[0].page, next);
                assert_eq!(matching[1].page, next + 1);
                next += 2;
            }
        }
    }
    for page in [18_u32, 19] {
        assert!(free(&case.before, page) && free(&case.expected, page));
    }
    for page in [28_usize, 29] {
        assert_eq!(
            &case.before[page * PAGE..(page + 1) * PAGE],
            &case.expected[page * PAGE..(page + 1) * PAGE]
        );
        assert!(!free(&case.before, page as u32) && !free(&case.expected, page as u32));
    }
    for page in case.eof..next {
        assert!(free(&case.before, page));
        assert!(!free(&case.expected, page));
    }
    assert!(free(&case.expected, next));
    if matches!(case.settings.mode, Mode::Full | Mode::Clear) {
        assert_eq!(case.added.len(), 8);
        let global = model::span(&case.expected, 1 << 8);
        assert_eq!(
            case.expected[global.start + 9],
            if case.eof == 30 { 0xc0 } else { 0 }
        );
    } else {
        assert_eq!(case.added.len(), 4);
        assert_eq!(case.tables.iter().filter(|table| table.fitted).count(), 1);
    }
}

#[test]
fn qualify_multitable_nullable_sources_and_complete_expected_images_before_preview() {
    for settings in model::settings() {
        qualify_case(&model::source(settings));
    }
}
