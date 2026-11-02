//! Whole indexed planning and fresh recovery qualification pay for all work.
use crate::{
    EditError, FixedScalarUpdate, InspectError, NativeFixedIntentGroup, NativeImage,
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent, RecoverableEditError,
    qualify_native_update_recovery,
};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, inspect_v1},
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as sources, Case, Recipe,
};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];
const RECIPES: [Recipe; 3] = [Recipe::UniqueSwap, Recipe::PromoteNull, Recipe::ManualTree];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(122)
}

fn cell(value: Option<i32>) -> Value {
    value.map(Value::Int32).unwrap_or(Value::Null)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    let requests = case
        .updates
        .iter()
        .map(|update| {
            FixedScalarUpdate::new(update.row, cell(update.expected), cell(update.replacement))
        })
        .collect();
    vec![NativeFixedIntentGroup::new("Id", requests, &mut budget(), &|| false).unwrap()]
}

fn literal_carrier(bytes: &mut Vec<u8>, value: Option<i32>) {
    match value {
        Some(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&u64::from(value as u32).to_le_bytes());
        }
        None => {
            bytes.push(0);
            bytes.extend_from_slice(&0_u64.to_le_bytes());
        }
    }
}

fn original(case: &Case) -> NativeUpdateIntent {
    // Independent original-intent DATA. No producer is needed to manufacture
    // the input to public recovery qualification.
    let length = 72 + 5 + 12 + 2 + 26 * case.updates.len();
    let mut bytes = Vec::from(*b"TCNI0004");
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&[4, 0, 0, 0]);
    bytes.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    bytes.extend_from_slice(&digest.finalize());
    for value in [5_u32, 1, case.updates.len() as u32, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"Items");
    for value in [2_u32, case.updates.len() as u32, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"Id");
    for update in &case.updates {
        bytes.extend_from_slice(&(update.row as u64).to_le_bytes());
        literal_carrier(&mut bytes, update.expected);
        literal_carrier(&mut bytes, update.replacement);
    }
    assert_eq!(bytes.len(), length);
    NativeUpdateIntent::decode(&bytes, &mut budget(), &|| false).unwrap()
}

struct ReadOnlyJournal {
    bytes: Cursor<Vec<u8>>,
}

impl ReadOnlyJournal {
    fn pending(case: &Case) -> Self {
        let after = case
            .expected
            .as_ref()
            .expect("positive independent outcome");
        assert_eq!(case.before.len(), after.len());
        // Literal V1 pending framing, independent of transaction serialization.
        let mut bytes = Vec::from(*b"TCIMG001");
        bytes.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
        let mut digest = Sha256::new();
        digest.update(&bytes);
        digest.update(&case.before);
        digest.update(after);
        bytes.extend_from_slice(&digest.finalize());
        bytes.extend_from_slice(&case.before);
        bytes.extend_from_slice(after);
        Self {
            bytes: Cursor::new(bytes),
        }
    }
}

impl Read for ReadOnlyJournal {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for ReadOnlyJournal {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for ReadOnlyJournal {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        panic!("native qualification must not write the journal")
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("native qualification must not flush the journal")
    }
}

impl DurableStorage for ReadOnlyJournal {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        panic!("native qualification must not sync the journal")
    }
}

fn planning_error(error: RecoverableEditError) -> InspectError {
    match error {
        RecoverableEditError::Edit(EditError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Inspect(error))
        | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Inspect(error))) => {
            error
        }
        error => panic!("expected typed inspection failure, got {error:?}"),
    }
}

fn recovery_error(error: NativeRecoveryQualificationError) -> InspectError {
    match error {
        NativeRecoveryQualificationError::Inspect(error)
        | NativeRecoveryQualificationError::Edit(EditError::Inspect(error))
        | NativeRecoveryQualificationError::Intent(NativeIntentError::Inspect(error))
        | NativeRecoveryQualificationError::Intent(NativeIntentError::Request(
            EditError::Inspect(error),
        )) => error,
        error => panic!("expected typed recovery inspection failure, got {error:?}"),
    }
}

fn execute(
    case: &Case,
    recovery: bool,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    // Public planning consumes an already captured image and caller-owned
    // groups. Public qualification consumes an already inspected journal and
    // decoded original intent. Those input operations use their own budget;
    // every fresh image/closure/record/index replay inside the measured public
    // operation uses work, including its final independent proof.
    if !recovery {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", groups(case), work, cancelled)
            .map_err(planning_error)?;
        assert_eq!(plan.after_bytes(), case.expected.as_ref().unwrap());
        assert_eq!(plan.changed_rows(), case.changed_rows);
        return Ok(());
    }

    let mut journal = ReadOnlyJournal::pending(case);
    let saved = journal.bytes.get_ref().clone();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let result = qualify_native_update_recovery(checked, original(case), scope(), work, cancelled)
        .map(drop)
        .map_err(recovery_error);
    assert_eq!(journal.bytes.get_ref(), &saved);
    result
}

#[test]
fn indexed_public_planning_and_recovery_require_each_exact_resource_pool() {
    for header in [5, 6] {
        for recipe in RECIPES {
            let case = sources::source(header, recipe);
            for recovery in [false, true] {
                let mut measured = budget();
                let initial = POOLS.map(|resource| measured.remaining(resource));
                execute(&case, recovery, &mut measured, &|| false).unwrap();
                let costs: [u64; 3] =
                    std::array::from_fn(|index| initial[index] - measured.remaining(POOLS[index]));
                assert!(costs.iter().all(|cost| *cost > 0));

                let mut exact = ResourceBudget::new(costs[0], costs[1], costs[2]);
                execute(&case, recovery, &mut exact, &|| false).unwrap();
                for resource in POOLS {
                    assert_eq!(exact.remaining(resource), 0);
                }

                for (index, resource) in POOLS.into_iter().enumerate() {
                    let mut limits = costs;
                    limits[index] -= 1;
                    let mut insufficient = ResourceBudget::new(limits[0], limits[1], limits[2]);
                    let error = execute(&case, recovery, &mut insufficient, &|| false)
                        .expect_err("one missing resource cannot qualify a complete operation");
                    assert!(matches!(
                        error,
                        InspectError::Limit(error) if error.resource == resource
                    ));
                }
            }
        }
    }
}

#[test]
fn indexed_public_planning_and_recovery_cancel_at_entry_middle_and_terminal() {
    for header in [5, 6] {
        for recipe in RECIPES {
            let case = sources::source(header, recipe);
            for recovery in [false, true] {
                let polls = Cell::new(0_usize);
                execute(&case, recovery, &mut budget(), &|| {
                    polls.set(polls.get() + 1);
                    false
                })
                .unwrap();
                let terminal = polls.get();
                assert!(terminal > 2);

                for stop in [1, terminal / 2, terminal] {
                    polls.set(0);
                    let error = execute(&case, recovery, &mut budget(), &|| {
                        polls.set(polls.get() + 1);
                        polls.get() == stop
                    })
                    .expect_err("cancelled operation must not publish a complete capability");
                    assert_eq!(error, InspectError::Cancelled);
                    assert_eq!(polls.get(), stop);
                }
            }
        }
    }
}
