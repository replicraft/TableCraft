#![cfg(feature = "journal")]
//! Recorded planning uses the actual SOURCE-qualified recovery literal owner.
#[path = "qualified_recovery/model.rs"]
mod model;

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_ace::{
    EditError, IndexCheckStatus, Int32Update, NativeImage, NativeInt32EditIntent,
    RecoverableEditError, WriteBlocker, check_table, qualify_jet4_int32_recovery,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, RecoveryOutcome, inspect_v1},
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn image(bytes: &[u8], work: &mut ResourceBudget) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(63),
        work,
        &|| false,
    )
    .unwrap()
}

struct Memory {
    bytes: Cursor<Vec<u8>>,
    syncs: usize,
    fail_after: Option<u64>,
}

impl Memory {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            syncs: 0,
            fail_after: None,
        }
    }
}

impl Read for Memory {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for Memory {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for Memory {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if let Some(end) = self.fail_after {
            if self.bytes.position() >= end {
                return Err(io::ErrorKind::Other.into());
            }
            let count = input.len().min((end - self.bytes.position()) as usize);
            self.bytes.write(&input[..count])
        } else {
            self.bytes.write(input)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush is not a durable barrier");
    }
}

impl DurableStorage for Memory {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.syncs += 1;
        Ok(())
    }
}

#[test]
fn recorded_normal_plans_match_all_eight_original_images_and_keep_request_allocation() {
    for shape in model::SHAPES {
        let case = model::case(shape);
        assert_eq!(
            &case.before[case.changed_span.clone()],
            &10_i32.to_le_bytes()
        );
        assert_eq!(
            &case.after[case.changed_span.clone()],
            &case.value.to_le_bytes()
        );
        let mut work = budget();
        let requests = vec![Int32Update::new(0, 10, case.value)];
        let pointer = requests.as_ptr();
        let recorded = image(&case.before, &mut work)
            .plan_recoverable_int32_updates("Items", "Score", requests, &mut work, &|| false)
            .unwrap();
        assert_eq!(recorded.before_bytes(), case.before);
        assert_eq!(recorded.after_bytes(), case.after, "{shape:?}");
        assert_eq!(recorded.changed_rows(), usize::from(case.value != 10));
        assert_eq!(recorded.intent().updates().as_ptr(), pointer);
        recorded
            .intent()
            .verify_before(&case.before, &mut work, &|| false)
            .unwrap();
        let encoded = recorded.intent().encode(&mut work, &|| false).unwrap();
        let decoded = NativeInt32EditIntent::decode(&encoded, &mut work, &|| false).unwrap();
        assert_eq!(decoded.updates()[0].expected(), 10);
        assert_eq!(decoded.updates()[0].value(), case.value);

        let checked = check_table(
            &mut Cursor::new(recorded.after_bytes()),
            CatalogScope::new(63),
            "Items",
            &mut work,
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.indexes().len(), case.indexes);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        if let Some(index) = checked.indexes().first() {
            assert_eq!(index.index().columns()[0].descending(), case.descending);
        }
        let mut target = Memory::new(case.before.clone());
        let mut journal = Memory::new(Vec::new());
        let prepared = recorded
            .prepare(&mut target, &mut journal, &mut work, &|| false)
            .unwrap();
        assert_eq!(
            prepared.commit(&mut work, &|| false).unwrap(),
            usize::from(case.value != 10)
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &model::journal(&case.before, &case.after, model::Marker::Committed)
        );
        assert_eq!(target.syncs, 1);
        assert_eq!(journal.syncs, 2);
    }
}

#[test]
fn recorded_prepare_and_partial_marker_use_existing_protocol_and_qualified_recovery() {
    let case = model::case(model::Shape::AscendingAlias);
    let mut work = budget();
    let recorded = image(&case.before, &mut work)
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, case.value)],
            &mut work,
            &|| false,
        )
        .unwrap();
    let original_intent = recorded.intent().encode(&mut work, &|| false).unwrap();
    let pending = model::journal(&case.before, &case.after, model::Marker::Pending);
    let mut target = Memory::new(case.before.clone());
    let mut journal = Memory::new(Vec::new());
    let prepared = recorded
        .prepare(&mut target, &mut journal, &mut work, &|| false)
        .unwrap();
    drop(prepared);
    assert_eq!(target.bytes.get_ref(), &case.before);
    assert_eq!(journal.bytes.get_ref(), &pending);

    let recorded = image(&case.before, &mut work)
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, case.value)],
            &mut work,
            &|| false,
        )
        .unwrap();
    let mut journal = Memory::new(Vec::new());
    journal.fail_after = Some(pending.len() as u64 + 3);
    let prepared = recorded
        .prepare(&mut target, &mut journal, &mut work, &|| false)
        .unwrap();
    assert!(prepared.commit(&mut work, &|| false).is_err());
    assert_eq!(target.bytes.get_ref(), &case.after);
    assert_eq!(
        journal.bytes.get_ref(),
        &model::journal(&case.before, &case.after, model::Marker::Partial)
    );
    journal.fail_after = None;
    for _ in 0..2 {
        let checked = inspect_v1(&mut journal, &mut work, &|| false).unwrap();
        let intent = NativeInt32EditIntent::decode(&original_intent, &mut work, &|| false).unwrap();
        let qualified =
            qualify_jet4_int32_recovery(checked, intent, CatalogScope::new(63), &mut work, &|| {
                false
            })
            .unwrap();
        assert_eq!(
            qualified
                .recover(&mut target, &mut work, &|| false)
                .unwrap(),
            RecoveryOutcome::RolledBack
        );
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
}

#[test]
fn original_format_property_and_request_refusals_stay_native_owned() {
    for code in [5, 6] {
        let before = model::real_ace(code);
        assert!(matches!(
            image(&before, &mut budget()).plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![],
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Format
            )))
        ));
    }
    let case = model::unknown_property();
    assert!(matches!(
        image(&case.before, &mut budget()).plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 12)],
            &mut budget(),
            &|| false
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::Properties
        )))
    ));
    let case = model::case(model::Shape::Plain);
    for requests in [
        vec![Int32Update::new(0, 11, 12)],
        vec![Int32Update::new(usize::MAX, 10, 12)],
        vec![Int32Update::new(0, 10, 12), Int32Update::new(0, 10, 12)],
    ] {
        assert!(matches!(
            image(&case.before, &mut budget()).plan_recoverable_int32_updates(
                "Items",
                "Score",
                requests,
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
    }
}
