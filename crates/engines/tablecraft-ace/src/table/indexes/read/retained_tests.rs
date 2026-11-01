//! Strict retained checks reuse independent original trees without staging edits.
use super::*;
use crate::{IndexCheckStatus, NativeImage, check_table};
use std::{cell::Cell, io::Cursor};
use tablecraft_testkit::access::canonical_ordinary_indexed::{self as sources, Case};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const CASES: [Case; 3] = [
    Case::RetainedTree,
    Case::AliasHistory,
    Case::NullableOrdinary,
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(181),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn retained(
    original: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<super::super::TableCheck, InspectError> {
    super::super::check::check_table_retained(
        &mut original.reader(),
        original.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )
}

#[test]
fn strict_retained_checks_accept_complete_trees_aliases_and_null_keys() {
    for header in [5, 6] {
        for case in CASES {
            let mut source = sources::source(case);
            source.bytes[20] = header;
            let original = image(&source.bytes);
            let checked = retained(&original, source.table, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.rows(), source.rows.len());
            assert_eq!(checked.indexes().len(), source.indexes.len());
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| { index.status() == IndexCheckStatus::Checked })
            );
            assert_eq!(original.as_bytes(), source.bytes);
        }
    }
}

#[test]
fn strict_retained_check_rejects_orphan_owned_page_without_changing_default_read() {
    for header in [5, 6] {
        let mut source = sources::source(Case::UnreachableOwned);
        source.bytes[20] = header;
        let original = image(&source.bytes);
        let ordinary = check_table(
            &mut original.reader(),
            original.catalog().catalog().scope(),
            source.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            ordinary
                .indexes()
                .iter()
                .all(|index| { index.status() == IndexCheckStatus::Checked })
        );
        assert!(matches!(
            retained(&original, source.table, &mut budget(), &|| false),
            Err(InspectError::Malformed)
        ));
        assert_eq!(original.as_bytes(), source.bytes);
    }
}

#[test]
fn strict_retained_check_pays_exact_pools_and_honors_all_cancel_boundaries() {
    for case in [Case::RetainedTree, Case::AliasHistory] {
        let source = sources::source(case);
        let original = image(&source.bytes);
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0_usize);
        drop(
            retained(&original, source.table, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap(),
        );
        let costs: [u64; 3] =
            std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
        assert!(costs.iter().all(|cost| *cost > 0));
        let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
        drop(retained(&original, source.table, &mut exact, &|| false).unwrap());
        for resource in POOLS {
            assert_eq!(exact.remaining(resource), 0);
        }
        for (index, resource) in POOLS.into_iter().enumerate() {
            let mut limits = costs;
            limits[index] -= 1;
            let mut insufficient = ResourceBudget::new(limits[0], limits[1], limits[2]);
            assert!(matches!(
                retained(&original, source.table, &mut insufficient, &|| false),
                Err(InspectError::Limit(error)) if error.resource == resource
            ));
        }
        let terminal = polls.get();
        assert!(terminal > 2);
        for stop in [1, terminal / 2, terminal] {
            polls.set(0);
            assert!(matches!(
                retained(&original, source.table, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                }),
                Err(InspectError::Cancelled)
            ));
            assert_eq!(polls.get(), stop);
        }
        assert_eq!(original.as_bytes(), source.bytes);
    }
}
