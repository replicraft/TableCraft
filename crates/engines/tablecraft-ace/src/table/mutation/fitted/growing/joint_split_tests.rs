//! Source-only qualification before any joint INDEX-growth producer is active.
pub(in crate::table::mutation::fitted) mod model;

use crate::test_support::base;
use crate::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
    inspect_table_index_counter, read_table,
    table::mutation::{
        gates, page_update::roots, rows::RowAssignment, variable_split::metadata::Metadata,
    },
};
use model::{Family, Shape};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

fn values(row: &model::Row, family: Family) -> Vec<Value> {
    vec![
        Value::Int32(row.id),
        Value::Int32(row.age),
        match &row.name {
            None => Value::Null,
            Some(bytes) if matches!(family, Family::Binary) => Value::Binary(bytes.clone()),
            Some(bytes) => Value::Text(String::from_utf8(bytes.clone()).unwrap()),
        },
    ]
}

struct Ordinal(usize);
impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

fn qualify(bytes: &[u8], rows: &[model::Row], family: Family) {
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        rows.iter()
            .map(|row| values(row, family))
            .collect::<Vec<_>>()
    );
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 64);
    assert_eq!(checked.indexes().len(), 4);
    for checked in checked.indexes() {
        assert_eq!(checked.status(), IndexCheckStatus::Checked);
        let count = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            checked.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            count.value(),
            if checked.index().data_number() == 0 {
                701
            } else {
                901
            }
        );
    }
    let constraints = check_table_constraints(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.fields().iter().all(|field| {
        field.required() != RuleCheckStatus::Unsupported
            && field.empty_text() != RuleCheckStatus::Unsupported
            && field.validation() != RuleCheckStatus::Unsupported
    }));
    assert!(constraints.relationships().is_empty());

    let mut work = budget();
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut work,
        &|| false,
    )
    .unwrap();
    let checked = gates::before_slot(&image, "Items", &mut work, &|| false).unwrap();
    let metadata = Metadata::capture_for_mixed_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2],
        &mut work,
        &|| false,
    )
    .unwrap();
    let ordinals = (0..64).map(Ordinal).collect::<Vec<_>>();
    let captured =
        roots::capture(&image, checked.schema(), &ordinals, &mut work, &|| false).unwrap();
    assert_eq!(captured.selected.len(), 64);
    for (ordinal, root) in captured.selected.iter().enumerate() {
        assert_eq!(root.reference, (12 << 8) | ordinal as u32);
        metadata
            .require_data_page((root.final_record.offset / 4096) as u32, &mut work, &|| {
                false
            })
            .unwrap();
    }
}

#[test]
fn qualify_joint_mixed_data_index_growth_sources_and_complete_expected_before_preview() {
    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for shape in [Shape::Root, Shape::Branch] {
                    let case = model::case(code, family, descending, shape);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);
                    let (data, indexes) = match (shape, family) {
                        (Shape::Root, Family::Legacy) => (vec![16, 17, 18], vec![19, 20, 21, 22]),
                        (Shape::Root, _) => (vec![16], vec![17, 18, 19, 20]),
                        (Shape::Branch, Family::Legacy) => (vec![21, 22, 23], vec![24, 25]),
                        (Shape::Branch, _) => (vec![21], vec![22, 23]),
                    };
                    assert_eq!(case.data_pages, data);
                    assert_eq!(case.index_pages, indexes);
                    assert_eq!(
                        &case.before[10 * 4096..11 * 4096],
                        &case.expected[10 * 4096..11 * 4096]
                    );
                    assert_eq!(case.data_pages.last().unwrap() + 1, case.index_pages[0]);
                }
            }
        }
    }
}
