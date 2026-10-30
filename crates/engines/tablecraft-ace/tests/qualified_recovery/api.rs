use super::{budget, model};
use model::{Marker, Shape};
use std::cell::Cell;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_ace::{
    EditError, InspectError, Int32Update, NativeInt32EditIntent, NativeRecoveryQualificationError,
    WriteBlocker, qualify_jet4_int32_recovery,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, JournalError, RecoveryOutcome, inspect_v1},
};
use tablecraft_model::CatalogScope;

struct Storage {
    bytes: Cursor<Vec<u8>>,
    writes: usize,
    fail_prefix: Option<usize>,
    fail_sync: bool,
}

impl Storage {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            writes: 0,
            fail_prefix: None,
            fail_sync: false,
        }
    }
}

impl Read for Storage {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for Storage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for Storage {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if let Some(prefix) = self.fail_prefix.take() {
            self.bytes.write_all(&input[..prefix.min(input.len())])?;
            return Err(io::Error::other("injected recovery prefix failure"));
        }
        self.bytes.write(input)
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("flush must not substitute for storage sync")
    }
}

impl DurableStorage for Storage {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        if std::mem::take(&mut self.fail_sync) {
            return Err(IoError::Io(io::ErrorKind::Other));
        }
        Ok(())
    }
}

fn intent(before: &[u8], expected: i32, value: i32) -> NativeInt32EditIntent {
    NativeInt32EditIntent::new(
        "Items",
        "Score",
        vec![Int32Update::new(0, expected, value)],
        before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn jet4_recovery_reproduces_literal_images_and_preserves_committed_targets() {
    for shape in model::SHAPES {
        let case = model::case(shape);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut journal = Storage::new(model::journal(&case.before, &case.after, marker));
            let original_journal = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_jet4_int32_recovery(
                checked,
                intent(&case.before, 10, case.value),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut target = Storage::new(case.after.clone());
            let outcome = qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            if matches!(marker, Marker::Committed) {
                assert_eq!(outcome, RecoveryOutcome::Committed);
                assert_eq!(target.bytes.get_ref(), &case.after);
                assert_eq!(target.writes, 0);
            } else {
                assert_eq!(outcome, RecoveryOutcome::RolledBack);
                assert_eq!(target.bytes.get_ref(), &case.before);
                assert!(target.writes > 0);
            }
            assert_eq!(journal.bytes.get_ref(), &original_journal);
            assert_eq!(journal.writes, 0);
        }
    }
}

#[test]
fn jet4_recovery_rejects_foreign_format_properties_and_unrelated_after_bytes() {
    for code in [5, 6] {
        let before = model::real_ace(code);
        let mut journal = Storage::new(model::journal(&before, &before, Marker::Pending));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_jet4_int32_recovery(
                checked,
                intent(&before, -1, 12),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::Edit(
                EditError::Unsupported(WriteBlocker::Format)
            ))
        ));
        assert_eq!(journal.writes, 0);
    }
    let unknown = model::unknown_property();
    let mut journal = Storage::new(model::journal(
        &unknown.before,
        &unknown.after,
        Marker::Pending,
    ));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_jet4_int32_recovery(
            checked,
            intent(&unknown.before, 10, 12),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        ),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(_)
        ))
    ));

    let mut case = model::case(Shape::Plain);
    case.after[13 * 4096] ^= 1;
    let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Committed));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_jet4_int32_recovery(
            checked,
            intent(&case.before, 10, 12),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(journal.writes, 0);
}

#[test]
fn jet4_recovery_original_before_and_typed_requests_remain_authority() {
    let case = model::case(Shape::Plain);
    for (expected, value, wrong_before) in [(11, 12, false), (10, 13, false), (10, 12, true)] {
        let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Pending));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let mut bound_before = case.before.clone();
        if wrong_before {
            bound_before[13 * 4096] ^= 1;
        }
        let error = qualify_jet4_int32_recovery(
            checked,
            intent(&bound_before, expected, value),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("original request or before binding must refuse");
        if wrong_before {
            assert!(matches!(error, NativeRecoveryQualificationError::Intent(_)));
        } else if expected == 11 {
            assert!(matches!(
                error,
                NativeRecoveryQualificationError::Edit(EditError::Conflict)
            ));
        } else {
            assert!(matches!(
                error,
                NativeRecoveryQualificationError::ImageMismatch
            ));
        }
        assert_eq!(journal.writes, 0);
    }
}

#[test]
fn jet4_recovery_partial_rollback_restarts_without_sql_or_journal_rewrite() {
    let case = model::case(Shape::WidePlain);
    let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Partial));
    let original_journal = journal.bytes.get_ref().clone();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_jet4_int32_recovery(
        checked,
        intent(&case.before, 10, case.value),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut target = Storage::new(case.after.clone());
    target.fail_prefix = Some(case.changed_span.start + 2);
    assert!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(
        &target.bytes.get_ref()[case.changed_span.clone()],
        &[10, 0, 0x34, 0x12]
    );

    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_jet4_int32_recovery(
        checked,
        intent(&case.before, 10, case.value),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::RolledBack
    );
    assert_eq!(target.bytes.get_ref(), &case.before);
    assert_eq!(journal.bytes.get_ref(), &original_journal);
    assert_eq!(journal.writes, 0);
}

#[test]
fn jet4_recovery_target_mismatch_refuses_before_any_target_write() {
    let case = model::case(Shape::Plain);
    let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_jet4_int32_recovery(
        checked,
        intent(&case.before, 10, case.value),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut target = Storage::new(case.after.clone());
    target.bytes.get_mut()[13 * 4096] ^= 1;
    let original = target.bytes.get_ref().clone();
    assert_eq!(
        qualified.recover(&mut target, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &original);
}

#[test]
fn jet4_recovery_qualification_pools_and_final_cancel_precede_capability() {
    let case = model::case(Shape::Plain);
    let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let mut observed = budget();
    let polls = Cell::new(0);
    drop(
        qualify_jet4_int32_recovery(
            checked,
            intent(&case.before, 10, case.value),
            CatalogScope::new(1),
            &mut observed,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let last_poll = polls.get();
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let original = budget().remaining(resource);
        let used = original - observed.remaining(resource);
        assert!(used > 0);
        for limit in [used - 1, used] {
            let mut limits = [1_000_000_000, 1_000_000, 1_000_000_000];
            limits[match resource {
                Resource::Bytes => 0,
                Resource::Rows => 1,
                Resource::Steps => 2,
            }] = limit;
            let mut allowance = ResourceBudget::new(limits[0], limits[1], limits[2]);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let result = qualify_jet4_int32_recovery(
                checked,
                intent(&case.before, 10, case.value),
                CatalogScope::new(1),
                &mut allowance,
                &|| false,
            );
            let succeeded = result.is_ok();
            drop(result);
            assert_eq!(succeeded, limit == used);
            assert_eq!(journal.writes, 0);
        }
    }
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let polls = Cell::new(0);
    assert!(matches!(
        qualify_jet4_int32_recovery(
            checked,
            intent(&case.before, 10, case.value),
            CatalogScope::new(1),
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == last_poll
            },
        ),
        Err(NativeRecoveryQualificationError::Inspect(
            InspectError::Cancelled
        ))
    ));
    assert_eq!(journal.writes, 0);
}

#[test]
fn jet4_recovery_empty_intent_retains_source_gates_and_complete_image_binding() {
    let case = model::case(Shape::NoOpPlain);
    let mut journal = Storage::new(model::journal(&case.before, &case.after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let original = NativeInt32EditIntent::new(
        "Items",
        "Score",
        Vec::new(),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let qualified = qualify_jet4_int32_recovery(
        checked,
        original,
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut target = Storage::new(case.before.clone());
    assert_eq!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::RolledBack
    );
    assert_eq!(target.bytes.get_ref(), &case.before);

    for code in [5, 6] {
        let before = model::real_ace(code);
        let mut journal = Storage::new(model::journal(&before, &before, Marker::Pending));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let original = NativeInt32EditIntent::new(
            "Items",
            "Score",
            Vec::new(),
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            qualify_jet4_int32_recovery(
                checked,
                original,
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::Edit(
                EditError::Unsupported(WriteBlocker::Format)
            ))
        ));
    }
}
