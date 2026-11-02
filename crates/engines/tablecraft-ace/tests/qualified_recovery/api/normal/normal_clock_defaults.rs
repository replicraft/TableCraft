//! TCNA5 retains original presence and exactly one needed civil input.
#[path = "normal_clock_defaults/recovery.rs"]
mod recovery;
#[path = "normal_clock_defaults/resources.rs"]
mod resources;

use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::{
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_ace::{
    EditError, InsertFieldValue, NativeClockInsertIntent, NativeImage, OmittedRowInsert,
    WriteBlocker, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_testkit::fixtures::access::normal_clock_defaults::{self as fixtures, Case, Field};

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case<'_>) -> Vec<OmittedRowInsert> {
    case.requests()
        .into_iter()
        .map(|row| {
            OmittedRowInsert::new(
                row.into_iter()
                    .map(|field| match field {
                        Field::Omitted => InsertFieldValue::Omitted,
                        Field::Supplied(value) => InsertFieldValue::Supplied(value),
                    })
                    .collect(),
            )
        })
        .collect()
}

struct OperationClock {
    reading: ClockReading,
    allowed: bool,
    reads: AtomicUsize,
}

impl OperationClock {
    fn new(case: &Case<'_>) -> Self {
        Self {
            reading: ClockReading::new(case.clock_input()).unwrap(),
            allowed: case.needs_clock(),
            reads: AtomicUsize::new(0),
        }
    }
    fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }
}

impl Clock for OperationClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        if self.reads.fetch_add(1, Ordering::Relaxed) == 0 && self.allowed {
            Ok(self.reading)
        } else {
            Err(EvalError::ClockUnavailable)
        }
    }
}

fn wire(case: &Case<'_>) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(case.before);
    case.intent_wire(digest.finalize().into())
}

#[test]
#[ignore = "requires frozen clock source18 native/SDK qualification before matching runtime"]
fn exact_clock_plans_commit_fourteen_source_images_and_four_empty_cases_with_one_or_zero_reads() {
    let sources = sources();
    let mut contexts = 0;
    for case in sources.cases() {
        let provider = OperationClock::new(&case);
        let plan = image(case.before)
            .plan_recoverable_ordinary_clock_row_inserts(
                "Items",
                requests(&case),
                &provider,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(provider.reads(), usize::from(case.needs_clock()));
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), case.requests().len());
        assert_eq!(plan.intent().reading().is_some(), case.needs_clock());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        let decoded =
            NativeClockInsertIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        let mut target = Storage::new(case.before.to_vec());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            case.requests().len()
        );
        assert_eq!(target.bytes.get_ref().as_slice(), case.after);
        assert_eq!(provider.reads(), usize::from(case.needs_clock()));
        let after = image(target.bytes.get_ref());
        assert_eq!(
            read_table(
                &mut after.reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.rows()
        );
        contexts += 1;
    }
    assert_eq!(contexts, 18);
}

#[test]
#[ignore = "requires qualified clock sources; refusals must precede any callback"]
fn no_clock_api_stays_closed_and_wrong_supplied_carriers_or_capacity_never_sample() {
    let sources = sources();
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.needs_clock())
    {
        assert!(
            image(case.before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(&case),
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        let provider = OperationClock::new(&case);
        let invalid = vec![OmittedRowInsert::new(vec![
            InsertFieldValue::Supplied(Value::Text("wrong".into())),
            InsertFieldValue::Omitted,
        ])];
        assert!(
            image(case.before)
                .plan_recoverable_ordinary_clock_row_inserts(
                    "Items",
                    invalid,
                    &provider,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(provider.reads(), 0);
        let mut saturated = case.before.to_vec();
        saturated[8 * 4096 + 12..8 * 4096 + 14].copy_from_slice(&255_u16.to_le_bytes());
        saturated[8 * 4096 + 2..8 * 4096 + 4].copy_from_slice(&3557_u16.to_le_bytes());
        for slot in 1..255 {
            let at = 8 * 4096 + 14 + slot * 2;
            saturated[at..at + 2].copy_from_slice(&0x8ff1_u16.to_le_bytes());
        }
        let healthy = image(&saturated);
        healthy
            .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
            .unwrap();
        let provider = OperationClock::new(&case);
        assert!(
            healthy
                .plan_recoverable_ordinary_clock_row_inserts(
                    "Items",
                    requests(&case),
                    &provider,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(provider.reads(), 0);
    }
}

#[test]
#[ignore = "requires frozen source18; equal-width unknown stored call remains inert"]
fn unknown_stored_calls_refuse_in_metadata_selection_and_before_the_provider() {
    let sources = sources();
    for case in sources.cases().into_iter().filter(|case| {
        case.function == fixtures::Function::Date && case.recipe == fixtures::Recipe::LeapNoon
    }) {
        let needle = "Date()"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let positions = case
            .before
            .windows(needle.len())
            .enumerate()
            .filter(|(_, raw)| *raw == needle)
            .map(|(at, _)| at)
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        let mut unknown = case.before.to_vec();
        let replacement = "Time()"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        unknown[positions[0]..positions[0] + replacement.len()].copy_from_slice(&replacement);
        let input = image(&unknown);
        assert!(matches!(
            input.select_native_ordinary_clock_defaults(
                "Items",
                &requests(&case),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::Properties))
        ));
        let provider = OperationClock::new(&case);
        assert!(
            input
                .plan_recoverable_ordinary_clock_row_inserts(
                    "Items",
                    requests(&case),
                    &provider,
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(provider.reads(), 0);
    }
}
