//! Independent original content, never a writer or macro evaluator.
use super::capture;
use crate::{EditError, InspectError, NativeImage, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{self as model, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> Result<NativeImage, InspectError> {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(113),
        &mut budget(),
        &|| false,
    )
}

#[test]
fn complete_content_accepts_dynamic_roles_and_refuses_unclosed_content() {
    let mut accepted = 0;
    let mut refused = 0;

    for header in [5, 6] {
        for case in model::CASES {
            let mut source = model::source(case);
            source.bytes[20] = header;
            let original = match image(&source.bytes) {
                Ok(image) => image,
                Err(error) => {
                    assert!(!source.ordinary, "{}: {error:?}", case.id());
                    refused += 1;
                    continue;
                }
            };
            let result = capture(&original, &mut budget(), &|| false);

            if !source.ordinary {
                assert!(result.is_err(), "{} unexpectedly certified", case.id());
                refused += 1;
                continue;
            }

            let certificate = result.unwrap_or_else(|error| panic!("{}: {error:?}", case.id()));
            for expected in &source.tables {
                let table = read_table(
                    &mut original.reader(),
                    CatalogScope::new(113),
                    &expected.name,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                certificate
                    .require_source(&original, table.schema(), &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(table.rows(), expected.rows, "{}", case.id());
            }
            assert_eq!(original.as_bytes(), source.bytes, "{}", case.id());
            accepted += 1;
        }
    }

    assert_eq!(accepted, 24);
    assert_eq!(refused, 34);
}

#[test]
fn content_certificate_binds_the_exact_image_and_actual_user_role() {
    let source = model::source(Case::Extrema);
    let original = image(&source.bytes).unwrap();
    let other = image(&source.bytes).unwrap();
    let certificate = capture(&original, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut original.reader(),
        CatalogScope::new(113),
        &source.tables[0].name,
        &mut budget(),
        &|| false,
    )
    .unwrap();

    assert!(matches!(
        certificate.require_source(&other, table.schema(), &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));

    let system = read_table(
        &mut original.reader(),
        CatalogScope::new(113),
        "MSysACEs",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        certificate.require_source(&original, system.schema(), &mut budget(), &|| false),
        Err(EditError::Preservation)
    ));
}

#[test]
fn incomplete_resource_or_cancelled_work_never_issues_a_certificate() {
    let source = model::source(Case::RelocatedMaps);
    let original = image(&source.bytes).unwrap();

    assert!(capture(&original, &mut budget(), &|| true).is_err());
    assert!(
        capture(
            &original,
            &mut ResourceBudget::new(0, 1_000_000, 1_000_000_000),
            &|| false,
        )
        .is_err()
    );
    assert!(
        capture(
            &original,
            &mut ResourceBudget::new(1_000_000_000, 0, 1_000_000_000),
            &|| false,
        )
        .is_err()
    );
    assert!(
        capture(
            &original,
            &mut ResourceBudget::new(1_000_000_000, 1_000_000, 0),
            &|| false,
        )
        .is_err()
    );

    capture(&original, &mut budget(), &|| false).unwrap();
    assert_eq!(original.as_bytes(), source.bytes);
}
