//! Complete retained-index certification pays for all work before returning.
use super::capture;
use crate::{EditError, InspectError, NativeImage};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed::{self as sources, Case};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const CASES: [Case; 2] = [Case::RetainedTree, Case::AliasHistory];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(118),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn retained_tree_and_alias_capture_require_each_exact_resource_pool() {
    for header in [5, 6] {
        for case in CASES {
            let mut source = sources::source(case);
            source.bytes[20] = header;
            let original = image(&source.bytes);
            let mut measured = budget();
            let initial = POOLS.map(|resource| measured.remaining(resource));
            drop(capture(&original, &mut measured, &|| false).unwrap());
            let costs: [u64; 3] =
                std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(costs.iter().all(|cost| *cost > 0));

            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            drop(capture(&original, &mut exact, &|| false).unwrap());
            for resource in POOLS {
                assert_eq!(exact.remaining(resource), 0);
            }

            for (index, resource) in POOLS.into_iter().enumerate() {
                let mut limits = costs;
                limits[index] -= 1;
                let mut insufficient = ResourceBudget::new(limits[0], limits[1], limits[2]);
                let error = capture(&original, &mut insufficient, &|| false)
                    .err()
                    .expect("incomplete work cannot issue a content certificate");
                assert!(matches!(
                    error,
                    EditError::Inspect(InspectError::Limit(error)) if error.resource == resource
                ));
                assert_eq!(original.as_bytes(), source.bytes);
            }
        }
    }
}

#[test]
fn retained_tree_and_alias_capture_cancel_at_entry_middle_and_terminal_poll() {
    for header in [5, 6] {
        for case in CASES {
            let mut source = sources::source(case);
            source.bytes[20] = header;
            let original = image(&source.bytes);
            let polls = Cell::new(0_usize);
            drop(
                capture(&original, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap(),
            );
            let terminal = polls.get();
            assert!(terminal > 2);

            for stop in [1, terminal / 2, terminal] {
                polls.set(0);
                let error = capture(&original, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                })
                .err()
                .expect("cancelled work cannot issue a content certificate");
                assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
                assert_eq!(polls.get(), stop);
                assert_eq!(original.as_bytes(), source.bytes);
            }
        }
    }
}
