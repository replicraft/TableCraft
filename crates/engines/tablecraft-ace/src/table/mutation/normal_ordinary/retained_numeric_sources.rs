//! Original-only admission and metadata capability, before any mutation call.
use crate::{EditError, InspectError, NativeImage};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_retained_numeric;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires both pinned retained-numeric original and expected directories"]
fn genuine_retained_numeric_sources_admit_before_mutation() {
    let cases = genuine_retained_numeric::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    for case in cases {
        for raw in [&case.before, &case.after] {
            let image = NativeImage::capture(
                &mut Cursor::new(raw),
                CatalogScope::new(182),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let source = super::capture(&image, case.table, &mut budget(), &|| false)
                .expect("complete original retained-index Source admission");
            assert_eq!(source.table.schema().name(), case.table);
            assert_eq!(source.indexes.indexes().len(), 4);
            image
                .check_native_ordinary_fixed_table(case.table, &mut budget(), &|| false)
                .expect("actual unindexed fixed target capability");
            assert_eq!(image.as_bytes(), raw);
        }
    }
}

#[test]
#[ignore = "requires both pinned retained-numeric original and expected directories"]
fn retained_numeric_source_limits_and_cancellation_never_publish_partial_admission() {
    let cases = genuine_retained_numeric::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    for case in cases.into_iter().filter(|case| case.table == "ByteKeys") {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            CatalogScope::new(182),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut measured = budget();
        let initial = pools.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0_usize);
        drop(
            super::capture(&image, case.table, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap(),
        );
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(pools[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        drop(super::capture(&image, case.table, &mut exact, &|| false).unwrap());
        for resource in pools {
            assert_eq!(exact.remaining(resource), 0);
        }

        for (index, resource) in pools.into_iter().enumerate() {
            let mut limits = costs;
            limits[index] -= 1;
            let mut insufficient = ResourceBudget::new(limits[0], limits[1], limits[2]);
            assert!(matches!(
                super::capture(&image, case.table, &mut insufficient, &|| false),
                Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == resource
            ));
        }

        let terminal = polls.get();
        assert!(terminal > 2);
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            assert!(matches!(
                super::capture(&image, case.table, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                }),
                Err(EditError::Inspect(InspectError::Cancelled))
            ));
            assert_eq!(polls.get(), stop);
        }
        assert_eq!(image.as_bytes(), case.before);
    }
}
