//! Retired surrounding bytes never substitute for exact target absence evidence.
use super::{NoRequiredTablePrograms, catalog, declarations, properties};
use crate::{
    EditError, NativeImage, inspect_table_schema,
    table::{
        mutation::{
            normal_ordinary,
            ordinary_content::definition::{self, RowDeclaration},
            preserved_source,
        },
        table_append::Inventory,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target,
    },
    canonical_retained_add_table::{self as appended, Target as Added},
    canonical_table_drop::current::{self as raw, History},
};

const SCOPE: CatalogScope = CatalogScope::new(3211);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 128 * 1024 * 1024)
}

fn source(retired: bool, target: Added) -> Vec<u8> {
    let original = raw::source(
        Case::MixedBinary,
        Target::ShortText,
        true,
        History::Inserted,
    );
    let before = if retired {
        raw::after(&original)
    } else {
        original.bytes
    };
    appended::append(&before, "More", target)
}

fn image(bytes: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_insert_requires_its_exact_image_and_purpose_without_promoting_legacy_sources() {
    for retired in [false, true] {
        for target in [Added::GeneratedNumber, Added::GeneratedText] {
            let before = source(retired, target);
            let original = image(&before, SCOPE);
            let schema = inspect_table_schema(
                &mut original.reader(),
                SCOPE,
                "More",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let inventory = Inventory::capture(&original, &mut budget(), &|| false)
                .expect("retained inventory phase");
            let selected =
                catalog::validate_retained(&original, &schema, &inventory, &mut budget(), &|| {
                    false
                })
                .expect("retained catalog phase");
            if selected.properties_present {
                properties::capture_current_schema(&original, &schema, &mut budget(), &|| false)
                    .expect("retained target properties phase");
            }
            declarations::capture(&original, &mut budget(), &|| false)
                .expect("retained declaration census phase");
            definition::validate_rows(
                &original,
                &schema,
                RowDeclaration::CurrentSchema,
                &mut budget(),
                &|| false,
            )
            .expect("retained target definition phase");
            let certificate = NoRequiredTablePrograms::capture_retained_insert(
                &original,
                &schema,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            certificate
                .require_retained_insert_source(&original, &schema, &mut budget(), &|| false)
                .unwrap();
            let captured = preserved_source::capture_retained_insert(
                &original,
                "More",
                &certificate,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(captured.table.schema().definition_page(), 11);
            assert_eq!(captured.table.schema().columns().len(), 2);
            assert!(captured.table.rows().is_empty());
            assert!(captured.records.is_empty());
            assert_eq!(captured.indexes.indexes().len(), 1);
            assert!(matches!(
                certificate.require_source(&original, &schema, &mut budget(), &|| false),
                Err(EditError::Preservation)
            ));
            assert!(matches!(
                certificate.require_current_insert_source(
                    &original,
                    &schema,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            assert!(
                NoRequiredTablePrograms::capture_generated_insert(
                    &original,
                    &schema,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            let other = image(&before, SCOPE);
            assert!(matches!(
                certificate
                    .require_retained_insert_source(&other, &schema, &mut budget(), &|| false),
                Err(EditError::Preservation)
            ));
            let inventory = Inventory::capture(&original, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                catalog::validate_retained(&other, &schema, &inventory, &mut budget(), &|| false),
                Err(EditError::Preservation)
            ));
            if retired {
                let standard =
                    NativeImage::capture(&mut Cursor::new(&before), SCOPE, &mut budget(), &|| {
                        false
                    })
                    .unwrap();
                let schema = inspect_table_schema(
                    &mut standard.reader(),
                    SCOPE,
                    "More",
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(
                    NoRequiredTablePrograms::capture_generated_insert(
                        &standard,
                        &schema,
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
            }
            assert_eq!(original.as_bytes(), before);
        }
    }
}

#[test]
fn target_program_presence_and_retired_owner_corruption_refuse_the_same_source_entry() {
    let before = source(true, Added::GeneratedText);
    let mut code = before.clone();
    let page = 4 * 4096;
    let count = usize::from(u16::from_le_bytes(
        code[page + 12..page + 14].try_into().unwrap(),
    ));
    let end = usize::from(
        u16::from_le_bytes(
            code[page + 14 + (count - 2) * 2..page + 16 + (count - 2) * 2]
                .try_into()
                .unwrap(),
        ) & 0x1fff,
    );
    // The newly appended direct native17 catalog row has three presence bytes.
    code[page + end - 3 + 13 / 8] |= 1 << (13 % 8);
    let mut owner = before;
    owner[8 * 4096 + 4..8 * 4096 + 8].copy_from_slice(&2_u32.to_le_bytes());
    for bytes in [code, owner] {
        let original = image(&bytes, SCOPE);
        assert!(
            normal_ordinary::capture_retained_insert(&original, "More", &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(original.as_bytes(), bytes);
    }
}

#[test]
fn retained_insert_source_keeps_explicit_resource_pools_and_cancellation() {
    let before = source(true, Added::GeneratedNumber);
    let original = image(&before, SCOPE);
    assert!(matches!(
        normal_ordinary::capture_retained_insert(&original, "More", &mut budget(), &|| true),
        Err(EditError::Inspect(crate::InspectError::Cancelled))
    ));
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut work = ResourceBudget::new(
            if resource == Resource::Bytes {
                0
            } else {
                256 * 1024 * 1024
            },
            if resource == Resource::Rows {
                0
            } else {
                1_000_000
            },
            if resource == Resource::Steps {
                0
            } else {
                128 * 1024 * 1024
            },
        );
        assert!(
            normal_ordinary::capture_retained_insert(&original, "More", &mut work, &|| false)
                .is_err()
        );
        assert_eq!(work.remaining(resource), 0);
    }
}
