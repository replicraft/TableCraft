#![cfg(feature = "journal")]

#[path = "qualified_recovery/model.rs"]
#[allow(
    dead_code,
    reason = "Reuse the actual qualified literal owner; its auxiliary refusal/rollback builders have separate consumers"
)]
mod model;

use model::{Marker, Shape};
use std::io::Cursor;
use tablecraft_ace::{FileFormat, NativeImage, check_table};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
fn qualify_native_completed_confirmation_images_before_new_confirmation() {
    for shape in model::SHAPES {
        let case = model::case(shape);
        for (image, marker) in [
            (&case.before, Marker::Pending),
            (&case.after, Marker::Committed),
        ] {
            let native = NativeImage::capture(
                &mut Cursor::new(image),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(native.catalog().header().format(), FileFormat::Jet4);
            assert_eq!(
                check_table(
                    &mut native.reader(),
                    CatalogScope::new(1),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                3
            );
            let journal = model::journal(&case.before, &case.after, marker);
            assert_eq!(&journal[48..48 + case.before.len()], &case.before);
            assert_eq!(
                &journal[48 + case.before.len()..48 + 2 * case.before.len()],
                &case.after
            );
        }
    }
    // A concrete independent mixed image differs from both complete literals.
    let case = model::case(Shape::WidePlain);
    let mut mixed = case.after.clone();
    mixed[case.changed_span.start..case.changed_span.start + 2].copy_from_slice(&[10, 0]);
    assert_ne!(mixed, case.before);
    assert_ne!(mixed, case.after);
}

#[path = "completed_qualified_confirmation/api.rs"]
mod api;
