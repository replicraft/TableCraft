//! Explicit inert verification over retained desktop-origin positive originals.
use super::binding::{self, Event};
use crate::NativeImage;
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
#[ignore = "requires exact retained private originals; inert reader only"]
fn bind_nine_original_native_event_references_and_preserve_unresolved_names() {
    let directory = PathBuf::from(
        std::env::var_os("TABLECRAFT_NATIVE_MACRO_ORIGINALS_DIR")
            .expect("explicit original directory"),
    );
    let scope = CatalogScope::new(17);
    let inputs = [
        (
            "DataMacros.accdb",
            "d7deffb496ce78211df6df15595f6fa85a245e4d4392e866baa06ccc373cd238",
            &[(82_u64, 17_u32, 35_u8, false, &[Event::AfterInsert][..])][..],
        ),
        (
            "DataMacros_Example.accdb",
            "a0f83e9880b21068fcf321e00a962ad88231c9310fd63a6118c3ba59c50f78b3",
            &[
                (83, 17, 34, false, &[Event::AfterUpdate][..]),
                (89, 17, 35, false, &[Event::AfterUpdate][..]),
            ][..],
        ),
        (
            "OrderProcessingData_6.5.accdb",
            "409f18b89a7311e9aac6bf726968330263973cf33dd1a2cd8cc9b8100a6e7f4e",
            &[(347, 45, 27, true, &[][..]), (389, 45, 28, true, &[][..])][..],
        ),
        (
            "OrderProcessingData_6.5_FINAL.accdb",
            "1383d9e27ba7885b9173cc871410fe7c5be7f18dc21b478e9467b52eddc0a8f4",
            &[
                (
                    348,
                    45,
                    28,
                    true,
                    &[
                        Event::Named,
                        Event::BeforeChange,
                        Event::AfterUpdate,
                        Event::AfterDelete,
                        Event::AfterInsert,
                    ][..],
                ),
                (390, 45, 29, true, &[Event::Named][..]),
            ][..],
        ),
    ];
    let mut references = 0;
    for (file, hash, tables) in inputs {
        let path = directory.join(file);
        let bytes = fs::read(&path).unwrap();
        let expected: [u8; 32] = std::array::from_fn(|index| {
            u8::from_str_radix(&hash[2 * index..2 * index + 2], 16).unwrap()
        });
        assert_eq!(Sha256::digest(&bytes).as_slice(), expected);
        let image = NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget(), &|| false)
            .unwrap();
        for (id, page, slot, unresolved, expected) in tables {
            let bound =
                binding::capture(&image, scope.object_id(*id), &mut budget(), &|| false).unwrap();
            let extra = bound.coverage.table_extra();
            assert_eq!(extra.native_id(), *id as i32);
            assert_eq!(extra.parent_id(), 251_658_241);
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (*page, *slot));
            assert_eq!(bound.unresolved, *unresolved);
            let events = bound
                .references
                .iter()
                .map(|reference| reference.event)
                .collect::<Vec<_>>();
            assert!(events == *expected);
            references += events.len();
            if *id == 348 {
                let change = &bound.references[1];
                assert_eq!((change.group, change.property), (1, 0));
                let property = &extra.groups()[change.group].properties().unwrap()[change.property];
                assert_eq!(property.name(), "ValidateUpdate");
                assert_eq!(
                    bound.references[0].named.as_deref(),
                    Some("UpdateOrderAmount2")
                );
            }
        }
        assert_eq!(image.as_bytes(), bytes);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(references, 9);
}
