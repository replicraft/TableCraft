//! A single-field delete action feeds a separate two-component UPDATE key.
#[path = "partial_tuple/model.rs"]
mod literal;

use super::model;
use std::io::Cursor;
use tablecraft_ace::{IndexCheckStatus, check_table};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn partial_tuple_sources_and_complete_expected_images_qualify_before_preview() {
    for code in [5, 6] {
        for original_partial in [false, true] {
            let source = literal::source(code, original_partial);
            let expected = literal::expected(&source);
            model::before(&source);
            model::qualify(&expected);
            let checked = check_table(
                &mut Cursor::new(&source.bytes),
                CatalogScope::new(1),
                "Items",
                &mut model::budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            assert!(checked.indexes().iter().any(|index| {
                index.index().columns().len() == 1 && index.index().columns()[0].number() == 1
            }));
            assert!(checked.indexes().iter().any(|index| {
                index
                    .index()
                    .columns()
                    .iter()
                    .map(|column| column.number())
                    .collect::<Vec<_>>()
                    == [2, 1]
                    && index.index().constraints().is_some_and(|rules| {
                        rules.unique() && !rules.required() && !rules.primary_key()
                    })
            }));
            assert_eq!(source.rows[0][1][1], Value::Int32(20));
            assert_eq!(source.rows[2][0][1], Value::Int32(20));
            assert_eq!(expected.rows[0][1][1], Value::Null);
            assert_eq!(expected.rows[2][0][1], Value::Null);
            for target in [0, 2] {
                let row = if target == 0 { 1 } else { 0 };
                assert_eq!(expected.rows[target][row][2], source.rows[target][row][2]);
            }
        }
    }
}

#[test]
fn cascade_matches_partial_original_keys_and_retains_the_unchanged_component() {
    for code in [5, 6] {
        for original_partial in [false, true] {
            let source = literal::source(code, original_partial);
            let expected = literal::expected(&source);
            model::before(&source);
            model::qualify(&expected);
            let candidate = model::run(&source, &[(1, &[1])]).unwrap();
            model::verify(&source, &expected, &candidate);
            assert_eq!(candidate.deleted_rows(), 1);
        }
    }
}
