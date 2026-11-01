//! Independent ordinary-content inputs, qualified through the existing reader.
//! These checks never issue macro-absence authority or invoke a writer.
use crate::{NativeImage, check_table, check_table_constraints, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_ordinary_content::{self as model, Source};

const SCOPE: CatalogScope = CatalogScope::new(119);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(header: u8, case: model::Case) -> Source {
    assert!(matches!(header, 5 | 6));
    let mut source = model::source(case);
    // Independent wire input: the stored selector is a little-endian word at
    // byte 20. This changes no declaration, row, allocation or expected value.
    source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    source
}

#[test]
fn ordinary_source_cases_keep_their_explicit_dispositions_before_any_writer() {
    for header in [5, 6] {
        let mut ordinary = 0;
        let mut adversarial = 0;

        for case in model::CASES {
            let source = source(header, case);
            assert!(!source.tables.is_empty(), "{}", case.id());
            assert_eq!(&source.bytes[20..24], &u32::from(header).to_le_bytes());

            if source.ordinary {
                ordinary += 1;
            } else {
                adversarial += 1;
            }
        }

        assert_eq!((ordinary, adversarial), (12, 17));
    }
}

#[test]
fn all_twenty_four_ordinary_views_match_independent_tables_and_values() {
    let mut views = 0;
    let mut tables = 0;

    for header in [5, 6] {
        for case in model::CASES {
            let source = source(header, case);
            if !source.ordinary {
                continue;
            }

            let mut work = budget();
            let image =
                NativeImage::capture(&mut Cursor::new(&source.bytes), SCOPE, &mut work, &|| false)
                    .unwrap_or_else(|error| panic!("header {header}, {}: {error}", case.id()));

            for expected in &source.tables {
                let checked = check_table(
                    &mut image.reader(),
                    SCOPE,
                    &expected.name,
                    &mut work,
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("header {header}, {}: {error}", case.id()));
                assert_eq!(checked.schema().definition_page(), expected.definition);
                assert_eq!(checked.rows(), expected.rows.len());
                assert!(checked.indexes().is_empty());
                assert_eq!(
                    checked
                        .schema()
                        .columns()
                        .iter()
                        .map(|column| column.name())
                        .collect::<Vec<_>>(),
                    expected
                        .columns
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                );

                let actual = read_table(
                    &mut image.reader(),
                    SCOPE,
                    &expected.name,
                    &mut work,
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("header {header}, {}: {error}", case.id()));
                assert_eq!(actual.rows(), expected.rows, "{}", case.id());

                let constraints = check_table_constraints(
                    &mut image.reader(),
                    SCOPE,
                    &expected.name,
                    &mut work,
                    &|| false,
                )
                .unwrap_or_else(|error| panic!("header {header}, {}: {error}", case.id()));
                assert_eq!(constraints.rows(), expected.rows.len());
                assert!(constraints.relationships().is_empty());
                tables += 1;
            }

            assert_eq!(image.as_bytes(), source.bytes);
            views += 1;
        }
    }

    assert_eq!(views, 24);
    assert_eq!(tables, 56);
}

mod export;

mod inline_sources;

mod indexed_sources;

mod indexed_targets;

mod genuine_targets;

mod fitted_sources;

mod property_lval;

mod retained_numeric;

mod owned_growth;

mod owned_growth_second;

mod owned_growth_joint;

mod alternate_owned;

mod alternate_owned_second;

mod competing_owned;

mod eof_allocation;

mod compound_eof;

mod normal_delete;

mod forwarded_delete;

mod insert;

mod insert_multirow;

mod insert_feff;

mod indexed_delete;
mod indexed_insert;

mod indexed_association_originals;

mod indexed_delete_desc_nullable;
mod indexed_insert_association;

mod indexed_insert_desc_nullable;

mod constant_default_originals;

mod constant_default_sources;

mod inline_lval;
mod native17_creation;
