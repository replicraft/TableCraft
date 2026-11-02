//! Counter/index originals qualify independently of ordinary write admission.
use super::{SCOPE, budget};
use crate::{IndexCheckStatus, NativeImage, check_table, check_table_constraints, read_table};
use std::io::Cursor;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::{self as model, Source};

fn source(header: u8, case: model::Case) -> Source {
    assert!(matches!(header, 5 | 6));
    let mut source = model::source(case);
    source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    source
}

#[test]
fn all_original_counter_index_source_recipes_construct_before_any_writer() {
    for header in [5, 6] {
        let mut ordinary = 0;
        let mut adversarial = 0;
        for case in model::CASES {
            let original = source(header, case);
            assert!(!original.rows.is_empty(), "{}", case.id());
            assert_eq!(original.rows[0].len(), original.columns.len());
            if original.ordinary {
                ordinary += 1;
            } else {
                adversarial += 1;
            }
        }
        assert_eq!((ordinary, adversarial), (9, 15));
    }
}

#[test]
fn eighteen_counter_index_sources_match_rows_and_complete_native_index_checks() {
    let mut views = 0;
    let mut indexes = 0;
    for header in [5, 6] {
        for case in model::CASES {
            let original = source(header, case);
            if !original.ordinary {
                continue;
            }
            let image = NativeImage::capture(
                &mut Cursor::new(&original.bytes),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            let checked = check_table(
                &mut image.reader(),
                SCOPE,
                original.table,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            assert_eq!(checked.rows(), original.rows.len(), "{}", case.id());
            assert_eq!(checked.indexes().len(), original.indexes.len());
            let expected_flags = if original.last_counter.is_some() {
                7
            } else {
                3
            };
            assert_eq!(checked.schema().columns()[0].flags(), expected_flags);
            for (actual, expected) in checked.indexes().iter().zip(&original.indexes) {
                assert_eq!(actual.status(), IndexCheckStatus::Checked, "{}", case.id());
                assert_eq!(actual.index().name(), *expected);
                assert_eq!(Some(actual.index().root_page()), original.root);
                indexes += 1;
            }

            let rows = read_table(
                &mut image.reader(),
                SCOPE,
                original.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), original.rows, "{}", case.id());
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                original.table,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case.id()));
            assert_eq!(constraints.rows(), original.rows.len());
            assert!(constraints.relationships().is_empty());
            if let Some(last) = original.last_counter {
                assert_eq!(
                    &original.bytes[7 * 4096 + 20..7 * 4096 + 24],
                    &last.to_le_bytes()
                );
            }
            assert_eq!(image.as_bytes(), original.bytes);
            views += 1;
        }
    }
    assert_eq!(views, 18);
    assert_eq!(indexes, 18);
}

pub(in crate::table::mutation::normal_ordinary_sources) mod export;
