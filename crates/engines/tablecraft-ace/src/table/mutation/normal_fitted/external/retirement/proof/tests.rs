//! The independent descriptor proof has no fixed chunk quota or speculative reserve.
use super::references;
use crate::{EditError, InspectError, NativeImage};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::raw_lval_chain;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 100_000, 10_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1760),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn independent_retirement_walk_retains_all_1025_actual_references() {
    let chain = raw_lval_chain::source(1025);
    let image = image(&chain.image);
    assert_eq!(
        references(&image, &chain.descriptor, &mut budget(), &|| false).unwrap(),
        chain.references
    );
    assert!(matches!(
        references(
            &image,
            &chain.descriptor,
            &mut ResourceBudget::new(0, 100_000, 10_000_000),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(matches!(
        references(&image, &chain.descriptor, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn independent_walk_rejects_late_cycles_bad_owners_and_length_termination_mismatch() {
    for fault in 0..4 {
        let mut chain = raw_lval_chain::source(1025);
        let last = (chain.references[1024] >> 8) as usize * 4096;
        match fault {
            0 => {
                // The tail must terminate exactly when its original length ends.
                chain.image[last + 4091..last + 4095]
                    .copy_from_slice(&chain.references[0].to_le_bytes());
            }
            1 => {
                // Cycle before the declared end; every reference must be unique.
                let previous = (chain.references[1023] >> 8) as usize * 4096;
                chain.image[previous + 4091..previous + 4095]
                    .copy_from_slice(&chain.references[0].to_le_bytes());
            }
            2 => chain.image[last + 4..last + 8].copy_from_slice(b"FAKE"),
            _ => chain.descriptor[..4].copy_from_slice(&1026_u32.to_le_bytes()),
        }
        assert!(
            matches!(
                references(
                    &image(&chain.image),
                    &chain.descriptor,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ),
            "fault {fault}"
        );
    }
}

#[test]
fn one_chunk_walk_pays_only_its_real_reference_storage() {
    let chain = raw_lval_chain::source(1);
    let actual = references(
        &image(&chain.image),
        &chain.descriptor,
        &mut ResourceBudget::new(4096 + 64, 100, 100_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(actual, chain.references);
}
