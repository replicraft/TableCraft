//! Independent indexed-target original and literal outcome read qualification.
use crate::{IndexCheckStatus, NativeImage, check_table, check_table_constraints, read_table};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as model, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn qualify(bytes: &[u8], expected: &[Vec<Value>]) {
    let scope = CatalogScope::new(122);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), expected.len());
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let rows = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(rows.rows(), expected);
    check_table_constraints(&mut image.reader(), scope, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn ordinary_indexed_target_sources_and_literal_outcomes_qualify_before_writers() {
    let mut before_count = 0;
    let mut after_count = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let source = model::source(header, recipe);
            qualify(&source.before, &source.rows);
            before_count += 1;
            if let Some(after) = &source.expected {
                qualify(after, &source.final_rows);
                assert_eq!(source.before.len(), after.len());
                assert_eq!(&source.before[..8 * 4096], &after[..8 * 4096]);
                if source.changed_rows == 0 {
                    assert_eq!(after, &source.before);
                }
                after_count += 1;
            } else {
                assert!(source.refusal.is_some());
            }
            if recipe == Recipe::PrefixLoss {
                assert_eq!(
                    &source.before[9 * 4096 + 24..9 * 4096 + 26],
                    &4_u16.to_le_bytes()
                );
                let after = source.expected.as_ref().unwrap();
                assert_eq!(&after[9 * 4096 + 24..9 * 4096 + 26], &1_u16.to_le_bytes());
                assert_eq!(&after[9 * 4096 + 2..9 * 4096 + 4], &3599_u16.to_le_bytes());
            }
        }
    }
    assert_eq!((before_count, after_count), (32, 24));
}

mod export;
