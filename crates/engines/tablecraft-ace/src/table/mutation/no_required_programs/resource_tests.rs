//! Explicit original-only budget and cancellation gates; no producer is invoked.
use super::NoRequiredTablePrograms;
use crate::{EditError, InspectError, NativeImage, TableSchema, inspect_table_schema};
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(129);
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const SOURCE_DIRECTORY: &str = "TABLECRAFT_GENUINE_NO_REQUIRED_SOURCES_DIR";
const ORIGINALS: [(&str, usize, &str); 2] = [
    (
        "h5-distinct.accdb",
        405504,
        "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
    ),
    (
        "h6-distinct.accdb",
        475136,
        "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
    ),
];

#[derive(Clone, Copy, Debug)]
enum Operation {
    Certificate,
    Capability,
}

const OPERATIONS: [Operation; 2] = [Operation::Certificate, Operation::Capability];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn directory() -> std::path::PathBuf {
    let directory = std::env::var_os(SOURCE_DIRECTORY)
        .expect("root must explicitly provide the retained genuine source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    directory.to_path_buf()
}

fn original(directory: &Path, filename: &str, length: usize, digest: &str) -> Vec<u8> {
    let bytes = std::fs::read(directory.join(filename)).unwrap();
    assert_eq!(bytes.len(), length);
    let actual = Sha256::digest(&bytes);
    let actual = actual
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, digest);
    bytes
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn target(image: &NativeImage) -> TableSchema {
    inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap()
}

fn run(
    operation: Operation,
    image: &NativeImage,
    target: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    match operation {
        Operation::Certificate => {
            let _certificate = NoRequiredTablePrograms::capture(image, target, budget, cancelled)?;
            Ok(())
        }
        Operation::Capability => {
            image.check_native_ordinary_fixed_table("Items", budget, cancelled)
        }
    }
}

#[test]
#[ignore = "requires both explicit pinned genuine originals; invokes no producer"]
fn genuine_certificate_and_full_capability_require_each_exact_resource_pool() {
    let directory = directory();
    for (filename, length, digest) in ORIGINALS {
        let bytes = original(&directory, filename, length, digest);
        let image = image(&bytes);
        let target = target(&image);
        for operation in OPERATIONS {
            // Original file capture and target-schema setup have separate
            // allowances. Measure only the actual operation under test.
            let mut measured = budget();
            let initial = POOLS.map(|resource| measured.remaining(resource));
            run(operation, &image, &target, &mut measured, &|| false).unwrap();
            let costs: [u64; 3] =
                std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
            assert!(
                costs.iter().all(|cost| *cost > 0),
                "{filename}: {operation:?}"
            );

            let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
            run(operation, &image, &target, &mut exact, &|| false).unwrap();
            for resource in POOLS {
                assert_eq!(exact.remaining(resource), 0, "{filename}: {operation:?}");
            }

            for (index, resource) in POOLS.into_iter().enumerate() {
                let mut limits = costs;
                limits[index] -= 1;
                let mut insufficient = ResourceBudget::new(limits[0], limits[1], limits[2]);
                let error = run(operation, &image, &target, &mut insufficient, &|| false)
                    .expect_err("incomplete work cannot issue a certificate or capability");
                assert!(
                    matches!(error, EditError::Inspect(InspectError::Limit(error)) if error.resource == resource),
                    "{filename}: {operation:?}: {resource:?}"
                );
                assert_eq!(image.as_bytes(), bytes);
            }
        }
        assert_eq!(image.as_bytes(), bytes);
    }
}

#[test]
#[ignore = "requires both explicit pinned genuine originals; invokes no producer"]
fn genuine_certificate_and_full_capability_cancel_at_entry_middle_and_terminal_poll() {
    let directory = directory();
    for (filename, length, digest) in ORIGINALS {
        let bytes = original(&directory, filename, length, digest);
        let image = image(&bytes);
        let target = target(&image);
        for operation in OPERATIONS {
            let polls = Cell::new(0_usize);
            run(operation, &image, &target, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap();
            let terminal = polls.get();
            assert!(terminal > 2);

            for stop in [1, terminal / 2, terminal] {
                polls.set(0);
                let error = run(operation, &image, &target, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == stop
                })
                .expect_err("cancelled work cannot issue a certificate or capability");
                assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
                assert_eq!(polls.get(), stop, "{filename}: {operation:?}");
                assert_eq!(image.as_bytes(), bytes);
            }
        }
    }
}
