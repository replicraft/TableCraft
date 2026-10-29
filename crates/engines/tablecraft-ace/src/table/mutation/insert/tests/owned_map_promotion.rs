//! Independent ordinary-owned promotion images qualify before actual preview use.
mod guards;
mod model;
use crate::{
    IndexCheckStatus, check_table, check_table_constraints,
    format::{allocation, reader::Reader},
    inspect_table_schema, read_table,
    table::mutation::gates,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn checked(bytes: &[u8], rows: &[Vec<Value>]) {
    let report = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), rows.len());
    assert!(
        report
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    gates::qualified(&constraints).unwrap();
    let snapshot = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows(), rows);
}

#[test]
fn literal_sources_and_owned_promotion_images_qualify_before_preview_execution() {
    for code in [5, 6] {
        for count in [1, 16] {
            let case = model::case(code, count);
            checked(&case.before, &case.old_rows);
            checked(&case.expected, &case.final_rows);
            let schema = inspect_table_schema(
                &mut Cursor::new(&case.expected),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut input = Cursor::new(case.expected.as_slice());
            let mut resources = budget();
            let mut reader = Reader {
                source: &mut input,
                header: schema.header,
                budget: &mut resources,
                cancelled: &|| false,
                reads: 0,
            };
            assert_eq!(
                allocation::pages(&mut reader, schema.layout.usage).unwrap(),
                case.owned
            );
            assert_eq!(&case.expected[521 * PAGE..521 * PAGE + 4], &[5, 1, 0, 0]);
            for excluded in [13, 513, 514, 515, 516, 517, 518, 519, 521] {
                assert_eq!(
                    case.expected[521 * PAGE + 4 + excluded / 8] & (1 << (excluded % 8)),
                    0
                );
            }
            assert_eq!(
                &case.before[512 * PAGE..513 * PAGE],
                &case.expected[512 * PAGE..513 * PAGE]
            );
            assert_eq!(
                &case.before[516 * PAGE..520 * PAGE],
                &case.expected[516 * PAGE..520 * PAGE]
            );
            assert_eq!(
                case.data,
                if count == 1 {
                    vec![520]
                } else {
                    vec![520, 522]
                }
            );
            assert_eq!(case.values.len(), count);
        }
    }
}
