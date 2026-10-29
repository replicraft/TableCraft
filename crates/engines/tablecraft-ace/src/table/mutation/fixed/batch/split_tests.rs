//! Original nullable populations and independent allocation-image read gates.
mod adversarial;
mod adversarial_model;
pub(in crate::table::mutation::fixed::batch) mod model;
pub(in crate::table::mutation::fixed::batch) mod orphans;
mod preview;

use crate::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
    read_table,
};
use model::{Case, PAGE, Row, Shape};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn qualify(bytes: &[u8], rows: &[Row], histories: [u32; 2]) {
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert_eq!(checked.indexes().len(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    for (position, index) in checked.indexes().iter().enumerate() {
        assert_eq!(index.index().data_number(), (position / 2) as u32);
        assert!(!index.index().constraints().unwrap().primary_key());
    }
    let report = check_table_constraints(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), rows.len());
    assert_eq!(report.fields().len(), 3);
    assert!(
        report
            .fields()
            .iter()
            .all(|field| field.required() != RuleCheckStatus::Unsupported
                && field.empty_text() != RuleCheckStatus::Unsupported
                && field.validation() != RuleCheckStatus::Unsupported)
    );
    assert_ne!(report.table_validation(), RuleCheckStatus::Unsupported);
    assert!(report.relationships().is_empty());
    let table = read_table(
        &mut image.reader(),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), rows.len());
    for (actual, expected) in table.rows().iter().zip(rows) {
        assert_eq!(
            actual,
            &vec![
                Value::Int32(expected.id),
                expected.left.map_or(Value::Null, Value::Int32),
                expected.right.map_or(Value::Null, Value::Int32),
            ]
        );
    }
    for (physical, history) in histories.into_iter().enumerate() {
        let offset = 10 * PAGE + 67 + physical * 12;
        assert_eq!(
            u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            history
        );
    }
}

fn qualify_case(case: &Case) {
    qualify(&case.before, &case.original, [700, 800]);
    qualify(&case.expected, &case.final_rows, case.final_history);
    assert_eq!(
        case.expected.len() - case.before.len(),
        case.added.len() * PAGE
    );
    assert_eq!(case.source_eof as usize * PAGE, case.before.len());
    for page in [12, 16, 17] {
        let at = page * PAGE;
        let count = usize::from(u16::from_le_bytes(
            case.before[at + 12..at + 14].try_into().unwrap(),
        ));
        assert_eq!(
            case.before[at..at + 14 + 2 * count],
            case.expected[at..at + 14 + 2 * count]
        );
    }
    if let Some(target) = case.target {
        let root = model::span(&case.before, case.original[0].reference);
        assert_eq!(case.before[root.clone()], case.expected[root]);
        let page = (target >> 8) as usize * PAGE;
        assert_eq!(case.before[page..page + 16], case.expected[page..page + 16]);
    }
    // Ordinary records never resize, and no DATA-owned or candidate bit changes.
    assert_eq!(
        case.before[11 * PAGE..12 * PAGE],
        case.expected[11 * PAGE..12 * PAGE]
    );
    assert_eq!(
        case.before[10 * PAGE + 16..10 * PAGE + 20],
        case.expected[10 * PAGE + 16..10 * PAGE + 20]
    );
    assert_eq!(
        case.before[10 * PAGE + 67..10 * PAGE + 71],
        case.expected[10 * PAGE + 67..10 * PAGE + 71]
    );
}

#[test]
fn qualify_nullable_split_batch_sources_and_complete_expected_images_before_preview() {
    for code in [5, 6] {
        for descending in [false, true] {
            for unique in [false, true] {
                for forwarded in [false, true] {
                    for shape in [
                        Shape::Mixed,
                        Shape::Empty,
                        Shape::Branch,
                        Shape::Clear,
                        Shape::BranchClear,
                    ] {
                        let case = model::source(code, shape, descending, unique, forwarded);
                        qualify_case(&case);
                        let expected_owners = if matches!(shape, Shape::BranchClear) {
                            vec![]
                        } else if matches!(shape, Shape::Branch) {
                            vec![1]
                        } else {
                            vec![0, 0, 1, 1]
                        };
                        assert_eq!(
                            case.added
                                .iter()
                                .map(|(_, owner)| *owner)
                                .collect::<Vec<_>>(),
                            expected_owners
                        );
                        assert!(
                            case.added
                                .iter()
                                .enumerate()
                                .all(|(position, (number, _))| *number
                                    == case.source_eof + position as u32)
                        );
                    }
                }
            }
        }
    }
}
