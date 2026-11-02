//! Literal original catalog roles test caller strictness, not parser rewriting.
use super::{capture, validate};
use crate::{
    EditError, InspectError, LongValueMapReferences, NativeImage, TableSchema,
    format::reader::Reader,
    inspect_table_schema,
    table::long_value::maps::{self, Declaration, Declarations},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_creation as sources;

const SCOPE: CatalogScope = CatalogScope::new(123);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(header: u8) -> NativeImage {
    let mut source = sources::source(sources::Case::One);
    source.bytes[20] = header;
    NativeImage::capture(
        &mut Cursor::new(source.bytes),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn schema(image: &NativeImage, table: &str) -> TableSchema {
    inspect_table_schema(&mut image.reader(), SCOPE, table, &mut budget(), &|| false).unwrap()
}

fn pair(number: usize, owned: u32, candidate: u32) -> Declaration {
    Declaration {
        number,
        references: LongValueMapReferences { owned, candidate },
    }
}

fn original() -> Declarations {
    // Original catalog declarations are native fields5..8 with exact map3
    // slots8..15. These literals are independent of the production parser.
    Declarations {
        complete: true,
        entries: vec![
            pair(5, 0x308, 0x309),
            pair(6, 0x30a, 0x30b),
            pair(7, 0x30c, 0x30d),
            pair(8, 0x30e, 0x30f),
        ],
    }
}

#[test]
fn all_local_original_declarations_are_consumed_before_admission() {
    for header in [5, 6] {
        let image = image(header);
        let before = image.as_bytes().to_vec();
        capture(&image, &mut budget(), &|| false).unwrap();
        validate(
            &schema(&image, "MSysObjects"),
            &original(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let empty = Declarations {
            complete: true,
            entries: Vec::new(),
        };
        validate(&schema(&image, "Items"), &empty, &mut budget(), &|| false).unwrap();
        assert_eq!(image.as_bytes(), before);
    }
}

#[test]
fn missing_duplicate_unknown_wrong_kind_and_zero_pairs_never_qualify() {
    for header in [5, 6] {
        let image = image(header);
        let catalog = schema(&image, "MSysObjects");
        for control in 0..8 {
            let mut declarations = original();
            match control {
                0 => declarations.complete = false,
                1 => declarations.entries.clear(),
                2 => {
                    declarations.entries.pop();
                }
                3 => declarations.entries[3].number = 7,
                4 => declarations.entries[3].number = 100,
                5 => declarations.entries[3].number = 0,
                6 => declarations.entries[3].references.owned = 0,
                7 => declarations.entries[3].references.candidate = 0,
                _ => unreachable!(),
            }
            assert!(
                validate(&catalog, &declarations, &mut budget(), &|| false).is_err(),
                "{control}"
            );
        }
        let ordinary = schema(&image, "Items");
        assert!(validate(&ordinary, &original(), &mut budget(), &|| false).is_err());
    }
}

#[test]
fn strict_declaration_comparisons_are_paid_and_cancellable() {
    for header in [5, 6] {
        let image = image(header);
        let catalog = schema(&image, "MSysObjects");
        let declarations = original();
        let mut measured = budget();
        let initial = measured.remaining(Resource::Steps);
        validate(&catalog, &declarations, &mut measured, &|| false).unwrap();
        let steps = initial - measured.remaining(Resource::Steps);
        assert!(steps > 2);
        let mut exact = ResourceBudget::new(0, 0, steps);
        validate(&catalog, &declarations, &mut exact, &|| false).unwrap();
        assert_eq!(exact.remaining(Resource::Steps), 0);
        let error = validate(
            &catalog,
            &declarations,
            &mut ResourceBudget::new(0, 0, steps - 1),
            &|| false,
        )
        .unwrap_err();
        assert!(
            matches!(error, EditError::Inspect(InspectError::Limit(error)) if error.resource == Resource::Steps)
        );

        let polls = Cell::new(0_usize);
        validate(&catalog, &declarations, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let terminal = polls.get();
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            let error = validate(&catalog, &declarations, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == stop
            })
            .unwrap_err();
            assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
            assert_eq!(polls.get(), stop);
        }
    }
}

#[test]
fn framing_complete_terminator_cannot_hide_missing_catalog_map_roles() {
    for header in [5, 6] {
        let mut source = sources::source(sources::Case::One);
        source.bytes[20] = header;
        let definition = 2 * 4096;
        let length = u32::from_le_bytes(
            source.bytes[definition + 8..definition + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        let first_pair = definition + length - 42;
        assert_eq!(&source.bytes[first_pair..first_pair + 2], &[5, 0]);
        assert_eq!(
            &source.bytes[definition + length - 2..definition + length],
            &[255, 255]
        );
        // Preserve every original byte except the first declaration tag. The
        // shared reader still reports a framing-complete empty tail here.
        source.bytes[first_pair..first_pair + 2].copy_from_slice(&[255, 255]);
        let image = NativeImage::capture(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let catalog = schema(&image, "MSysObjects");
        let mut input = image.reader();
        let mut allowance = budget();
        let mut reader = Reader {
            source: &mut input,
            header: image.catalog().header(),
            budget: &mut allowance,
            cancelled: &|| false,
            reads: 0,
        };
        let declarations = maps::declarations(&mut reader, &catalog).unwrap();
        assert!(declarations.complete);
        assert!(declarations.entries.is_empty());
        assert!(validate(&catalog, &declarations, &mut budget(), &|| false).is_err());
        assert!(capture(&image, &mut budget(), &|| false).is_err());
        assert_eq!(image.as_bytes(), source.bytes);
    }
}

#[test]
fn complete_field_coverage_does_not_authorize_shared_lval_or_ordinary_map_storage() {
    for header in [5, 6] {
        for alias in 0..3 {
            let mut source = sources::source(sources::Case::One);
            source.bytes[20] = header;
            let definition = 2 * 4096;
            let length = u32::from_le_bytes(
                source.bytes[definition + 8..definition + 12]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let first = definition + length - 42;
            let (destination, reference) = match alias {
                // Different declared fields retain distinct numbers while
                // sharing exactly the first field's owned map cell.
                0 => (first + 12, 0x308_u32),
                // Owned and candidate roles of one field cannot share a cell.
                1 => (first + 6, 0x308),
                // A catalog LVAL cannot reuse the user table's ordinary map.
                2 => (first + 2, 0x303),
                _ => unreachable!(),
            };
            source.bytes[destination..destination + 4].copy_from_slice(&reference.to_le_bytes());
            let image = NativeImage::capture(
                &mut Cursor::new(source.bytes),
                SCOPE,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let table = schema(&image, "MSysObjects");
            let mut input = image.reader();
            let mut work = budget();
            let mut reader = Reader {
                source: &mut input,
                header: image.catalog().header(),
                budget: &mut work,
                cancelled: &|| false,
                reads: 0,
            };
            let declarations = maps::declarations(&mut reader, &table).unwrap();
            assert!(declarations.complete);
            assert_eq!(declarations.entries.len(), 4);
            assert_eq!(
                declarations
                    .entries
                    .iter()
                    .map(|entry| entry.number)
                    .collect::<Vec<_>>(),
                [5, 6, 7, 8]
            );
            // This helper validates declaration bijection, not storage roles.
            validate(&table, &declarations, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                capture(&image, &mut budget(), &|| false),
                Err(EditError::Unsupported(crate::WriteBlocker::RowLayout))
            ));
        }
    }
}
