//! Both image directions exercise the actual shared fault-device owner.
use super::{Control, Device, MemoryStorage, budget};
use crate::{
    IoError,
    journal::{
        JournalError, RecoveryOutcome, ResizableDurableStorage, prepare_resizable_expected,
        recover_resizable,
        resizable::tests::goldens::{self, CASES},
    },
};
use std::{cell::RefCell, io::Cursor, rc::Rc};

impl ResizableDurableStorage for Device {
    fn resize_storage(&mut self, length: u64) -> Result<(), IoError> {
        let fails = self.before("resize")?;
        let requested = usize::try_from(length).unwrap();
        let current = self.bytes.get_ref().len();
        assert!(requested <= current, "complete image writes precede resize");
        let length = if fails && self.control.borrow().partial_resize {
            requested + (current - requested) / 2
        } else {
            requested
        };
        self.bytes.get_mut().truncate(length);
        Ok(self.after(fails, ())?)
    }
}

fn attempt(target: &mut Device, journal: &mut Device, before: &[u8], after: &[u8]) {
    if let Ok(prepared) =
        prepare_resizable_expected(target, journal, before, after, &mut budget(), &|| false)
    {
        let _ = prepared.commit(&mut budget(), &|| false);
    }
}

#[test]
fn resizable_barriers_order_image_write_resize_target_sync_then_marker() {
    for case in &CASES[..2] {
        let control = Rc::new(RefCell::new(Control::default()));
        let mut target = Device::new(case.before, "target", control.clone());
        let mut journal = Device::new(&[], "journal", control.clone());
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            case.before,
            case.after,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
        let control = control.borrow();
        let events = &control.events;
        let prepared = events
            .iter()
            .position(|event| *event == ("journal", "sync"))
            .unwrap();
        let write = events
            .iter()
            .position(|event| *event == ("target", "write"))
            .unwrap();
        let resize = events
            .iter()
            .position(|event| *event == ("target", "resize"))
            .unwrap();
        let sync = events
            .iter()
            .position(|event| *event == ("target", "sync"))
            .unwrap();
        let marker = events
            .iter()
            .enumerate()
            .find(|(index, event)| *index > sync && **event == ("journal", "write"))
            .unwrap()
            .0;
        assert!(prepared < write && write < resize && resize < sync && sync < marker);
        assert_eq!(target.durable, case.after);
        assert_eq!(journal.durable, goldens::bytes(case.committed));
    }
}

#[test]
fn every_resizable_commit_callback_failure_has_a_recognizable_recovery() {
    for case in CASES {
        let baseline = Rc::new(RefCell::new(Control::default()));
        attempt(
            &mut Device::new(case.before, "target", baseline.clone()),
            &mut Device::new(&[], "journal", baseline.clone()),
            case.before,
            case.after,
        );
        let calls = baseline.borrow().calls;

        for fail_at in 1..=calls {
            for after_effect in [false, true] {
                for partial_resize in [false, true] {
                    let control = Rc::new(RefCell::new(Control {
                        fail_at,
                        after_effect,
                        partial_resize,
                        ..Default::default()
                    }));
                    let mut target = Device::new(case.before, "target", control.clone());
                    let mut journal = Device::new(&[], "journal", control.clone());
                    attempt(&mut target, &mut journal, case.before, case.after);
                    assert!(control.borrow().hit, "{} callback {fail_at}", case.name);

                    for target_image in [target.bytes.get_ref(), &target.durable] {
                        for journal_image in [journal.bytes.get_ref(), &journal.durable] {
                            let mut target = MemoryStorage(Cursor::new(target_image.clone()));
                            let mut journal = MemoryStorage(Cursor::new(journal_image.clone()));
                            match recover_resizable(
                                &mut target,
                                &mut journal,
                                &mut budget(),
                                &|| false,
                            ) {
                                Ok(RecoveryOutcome::Committed) => {
                                    assert_eq!(target.0.get_ref(), case.after)
                                }
                                Ok(RecoveryOutcome::RolledBack) => {
                                    assert_eq!(target.0.get_ref(), case.before)
                                }
                                Err(JournalError::InvalidJournal) => {
                                    assert_eq!(target.0.get_ref(), case.before)
                                }
                                other => panic!("{} callback {fail_at}: {other:?}", case.name),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn interrupted_pending_recovery_and_committed_sync_can_repeat() {
    for case in &CASES[..2] {
        for committed in [false, true] {
            let mut record = goldens::bytes(case.committed);
            if !committed {
                record.truncate(record.len() - 7);
            }
            let baseline = Rc::new(RefCell::new(Control::default()));
            recover_resizable(
                &mut Device::new(case.after, "target", baseline.clone()),
                &mut Device::new(&record, "journal", baseline.clone()),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let calls = baseline.borrow().calls;

            for fail_at in 1..=calls {
                for after_effect in [false, true] {
                    for partial_resize in [false, true] {
                        let control = Rc::new(RefCell::new(Control {
                            fail_at,
                            after_effect,
                            partial_resize,
                            ..Default::default()
                        }));
                        let mut target = Device::new(case.after, "target", control.clone());
                        let mut journal = Device::new(&record, "journal", control.clone());
                        assert!(
                            recover_resizable(&mut target, &mut journal, &mut budget(), &|| false)
                                .is_err()
                        );
                        assert!(control.borrow().hit);
                        if committed {
                            assert!(
                                !control
                                    .borrow()
                                    .events
                                    .iter()
                                    .any(|event| matches!(*event, ("target", "write" | "resize")))
                            );
                        }

                        for target_image in [target.bytes.get_ref(), &target.durable] {
                            let mut target = MemoryStorage(Cursor::new(target_image.clone()));
                            let mut journal = MemoryStorage(Cursor::new(record.clone()));
                            let expected = if committed {
                                RecoveryOutcome::Committed
                            } else {
                                RecoveryOutcome::RolledBack
                            };
                            for _ in 0..2 {
                                assert_eq!(
                                    recover_resizable(
                                        &mut target,
                                        &mut journal,
                                        &mut budget(),
                                        &|| false
                                    )
                                    .unwrap(),
                                    expected
                                );
                                assert_eq!(
                                    target.0.get_ref(),
                                    if committed { case.after } else { case.before }
                                );
                                assert_eq!(journal.0.get_ref(), &record);
                            }
                        }
                    }
                }
            }
        }
    }
}
