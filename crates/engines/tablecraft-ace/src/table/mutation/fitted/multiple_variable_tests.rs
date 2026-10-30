//! Source-only qualification before any multi-variable record producer.
mod api;
mod model;
mod third;
mod third_model;

use super::growing::joint_split_tests::model as frames;
use crate::IndexCheckStatus;
use crate::NativeImage;
use crate::RuleCheckStatus;
use crate::check_table;
use crate::check_table_constraints;
use crate::format::allocation::MapRole;
use crate::format::allocation::RepresentedMap;
use crate::format::reader::Reader;
use crate::inspect_table_index_counter;
use crate::read_table;
use crate::table::mutation::gates;
use crate::table::mutation::page_update::roots;
use crate::table::mutation::rows::RowAssignment;
use crate::table::mutation::variable_split::metadata::Metadata;
use model::{Family, Placement, Recipe};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture as base;

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 1_000_000, 500_000_000)
}

struct Ordinal(usize);

impl RowAssignment for Ordinal {
    fn row(&self) -> usize {
        self.0
    }
}

fn qualify(bytes: &[u8], rows: &[model::Row], family: Family, context: &str) {
    let table = read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows().len(), rows.len(), "{context}: row count");
    for (ordinal, (actual, expected)) in table.rows().iter().zip(rows).enumerate() {
        assert!(
            actual == &model::values(expected, family),
            "{context}: typed row {ordinal}"
        );
    }
    let checked = check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert_eq!(checked.schema().columns().len(), 5);
    assert_eq!(checked.indexes().len(), 4);
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        let counter = inspect_table_index_counter(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            index.index().name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            counter.value(),
            if index.index().data_number() == 0 {
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
    for field in constraints.fields() {
        assert_ne!(field.required(), RuleCheckStatus::Unsupported);
        assert_ne!(field.empty_text(), RuleCheckStatus::Unsupported);
        assert_ne!(field.validation(), RuleCheckStatus::Unsupported);
    }
    assert_ne!(constraints.table_validation(), RuleCheckStatus::Unsupported);
    assert!(constraints.relationships().is_empty());

    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = gates::before_slot(&image, "Items", &mut budget(), &|| false)
        .unwrap_or_else(|error| panic!("{context}: source admission: {error}"));
    let mut global_budget = budget();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut global_budget,
        cancelled: &|| false,
        reads: 0,
    };
    RepresentedMap::read(&mut reader, 1 << 8, MapRole::GlobalFree).unwrap_or_else(|error| {
        panic!("{context}: GLOBAL page1 self-owner qualification: {error}")
    });
    let metadata = Metadata::capture_for_mixed_updates(
        &image,
        checked.schema(),
        checked.indexes(),
        &[1, 2, 3],
        &mut budget(),
        &|| false,
    )
    .unwrap_or_else(|error| panic!("{context}: complete source metadata census: {error}"));
    let requests = (0..rows.len()).map(Ordinal).collect::<Vec<_>>();
    let captured = roots::capture(&image, checked.schema(), &requests, &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(captured.selected.len(), rows.len());
    for root in captured.selected {
        assert_eq!(root.reference, (12 << 8) | root.update.0 as u32);
        metadata
            .require_data_page(12, &mut budget(), &|| false)
            .unwrap();
        metadata
            .require_data_page(
                (root.final_record.offset / 4096) as u32,
                &mut budget(),
                &|| false,
            )
            .unwrap();
    }
}

fn qualify_case(case: model::Case) {
    let name = format!(
        "header {} {:?} {:?}",
        case.before[20], case.family, case.placement
    );
    qualify(
        &case.before,
        &case.rows,
        case.family,
        &format!("{name} before"),
    );
    qualify(
        &case.expected,
        &case.final_rows,
        case.family,
        &format!("{name} expected"),
    );
    assert_eq!(
        &case.before[16 * model::PAGE..17 * model::PAGE],
        &case.expected[16 * model::PAGE..17 * model::PAGE]
    );
    assert_eq!(case.rows.len(), case.records.len());
    assert_eq!(case.rows.len(), case.final_records.len());
    assert!(!case.first_rows.is_empty() && !case.second_rows.is_empty());
    assert!(!case.changed.is_empty());
    for rows in [&case.first_rows, &case.second_rows, &case.fixed_rows] {
        assert!(rows.iter().all(|row| *row < case.rows.len()));
        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    }
    match case.placement {
        Placement::Fitted => {
            assert_eq!(case.before.len(), case.expected.len());
            assert!(case.data_pages.is_empty() && case.index_pages.is_empty());
        }
        Placement::Data => assert!(case.index_pages.is_empty()),
        Placement::DataIndexes => {
            assert_eq!(case.data_pages, [17]);
            assert_eq!(case.index_pages, [18, 19, 20, 21]);
            assert_eq!(case.expected.len(), 22 * model::PAGE);
        }
    }
    if case.fitted_refuses {
        assert!(case.final_records[0].len() > case.records[0].len());
        assert!(matches!(case.placement, Placement::Data));
    }
}

#[test]
fn qualify_multiple_variable_sources_and_complete_expected_before_preview() {
    let mut pairs = 0;
    for code in [5, 6] {
        for family in [Family::LegacyText, Family::ModernBinary, Family::Binary] {
            for descending in [false, true] {
                for recipe in [Recipe::Permutation, Recipe::NullShrinkGrow] {
                    qualify_case(model::fitted(code, family, descending, recipe));
                    pairs += 1;
                }
            }
        }
        for family in [Family::LegacyBinary, Family::ModernBinary] {
            for index_growth in [false, true] {
                qualify_case(model::growing(code, family, index_growth));
                pairs += 1;
            }
        }
        for exceeds in [false, true] {
            qualify_case(model::capacity(code, exceeds));
            pairs += 1;
        }
    }
    assert_eq!(pairs, 36);
}
