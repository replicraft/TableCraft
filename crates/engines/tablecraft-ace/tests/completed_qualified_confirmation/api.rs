use super::{budget, model};
use model::{Marker, Shape};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_ace::{Int32Update, NativeInt32EditIntent, qualify_jet4_int32_recovery};
use tablecraft_io::{
    IoError,
    journal::{DurableStorage, JournalError, RecoveryOutcome, inspect_v1},
};
use tablecraft_model::CatalogScope;

struct ReadOnly {
    bytes: Cursor<Vec<u8>>,
    syncs: usize,
}

impl ReadOnly {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Cursor::new(bytes),
            syncs: 0,
        }
    }
}

impl Read for ReadOnly {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for ReadOnly {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for ReadOnly {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        panic!("completed confirmation must never write")
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("completed confirmation must never flush")
    }
}

impl DurableStorage for ReadOnly {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.syncs += 1;
        Ok(())
    }
}

fn intent(before: &[u8], value: i32) -> NativeInt32EditIntent {
    NativeInt32EditIntent::new(
        "Items",
        "Score",
        vec![Int32Update::new(0, 10, value)],
        before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn native_completed_confirmation_checks_all_literal_images_without_writes() {
    for shape in model::SHAPES {
        let case = model::case(shape);
        for (marker, target, expected) in [
            (Marker::Pending, &case.before, RecoveryOutcome::RolledBack),
            (Marker::Partial, &case.before, RecoveryOutcome::RolledBack),
            (Marker::Committed, &case.after, RecoveryOutcome::Committed),
        ] {
            let bytes = model::journal(&case.before, &case.after, marker);
            let mut journal = ReadOnly::new(bytes.clone());
            let mut target = ReadOnly::new(target.clone());
            let original = target.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_jet4_int32_recovery(
                checked,
                intent(&case.before, case.value),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                expected
            );
            assert_eq!((target.syncs, journal.syncs), (1, 1));
            assert_eq!(target.bytes.get_ref(), &original);
            assert_eq!(journal.bytes.get_ref(), &bytes);
        }
    }
}

#[test]
fn native_completed_confirmation_refuses_opposite_or_mixed_images() {
    let case = model::case(Shape::WidePlain);
    let mut mixed = case.after.clone();
    mixed[case.changed_span.start..case.changed_span.start + 2].copy_from_slice(&[10, 0]);
    for (marker, image) in [
        (Marker::Pending, &case.after),
        (Marker::Partial, &case.after),
        (Marker::Committed, &case.before),
        (Marker::Pending, &mixed),
        (Marker::Committed, &mixed),
    ] {
        let mut journal = ReadOnly::new(model::journal(&case.before, &case.after, marker));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_jet4_int32_recovery(
            checked,
            intent(&case.before, case.value),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut target = ReadOnly::new(image.clone());
        assert_eq!(
            qualified.confirm_completed(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        );
        assert_eq!(target.bytes.get_ref(), image);
        assert_eq!((target.syncs, journal.syncs), (0, 0));
    }
}

#[test]
fn native_completed_confirmation_keeps_original_intent_and_normal_profile_authority() {
    let case = model::case(Shape::Plain);
    for wrong_intent in [true, false] {
        let mut after = case.after.clone();
        if !wrong_intent {
            after[0] ^= 1;
        }
        let mut journal = ReadOnly::new(model::journal(&case.before, &after, Marker::Committed));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_jet4_int32_recovery(
                checked,
                intent(&case.before, if wrong_intent { 13 } else { case.value }),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(journal.syncs, 0);
    }
    for code in [5, 6] {
        let before = model::real_ace(code);
        let mut journal = ReadOnly::new(model::journal(&before, &before, Marker::Pending));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_jet4_int32_recovery(
                checked,
                intent(&before, 12),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        assert_eq!(journal.syncs, 0);
    }
}
