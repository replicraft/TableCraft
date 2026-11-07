use super::protocol::{self, Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError, PublicationGuarantee};
use crate::IoError;
use std::cell::{Cell, RefCell};
use std::io::ErrorKind;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    Create,
    CreateUnbound,
    Write,
    WriteAndCleanup,
    PayloadSync,
    Verify,
    Collision,
    Unknown,
    ParentSync,
    Cleanup,
    CleanupAndBarrier,
    RequestCancelAfterPublication,
}

struct Model<'a> {
    fault: Fault,
    events: &'a RefCell<Vec<&'static str>>,
    cancelled: &'a Cell<bool>,
    identity: ParentIdentity,
    guarantee: PublicationGuarantee,
}

fn failure() -> IoError {
    IoError::Io(ErrorKind::Other)
}

impl Backend for Model<'_> {
    type Stage = u64;

    fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    fn guarantee(&self) -> PublicationGuarantee {
        self.guarantee
    }

    fn create(&self, _: &mut ResourceBudget, _: &dyn Fn() -> bool) -> Result<u64, CreateFailure> {
        self.events.borrow_mut().push("create");
        if matches!(self.fault, Fault::Create | Fault::CreateUnbound) {
            return Err(CreateFailure {
                cause: failure(),
                temporary_may_remain: self.fault == Fault::CreateUnbound,
            });
        }
        Ok(71)
    }

    fn write(
        &self,
        _: &mut u64,
        _: &[u8],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<(), IoError> {
        self.events.borrow_mut().push("write");
        if matches!(self.fault, Fault::Write | Fault::WriteAndCleanup) {
            return Err(failure());
        }
        Ok(())
    }

    fn sync_payload(&self, _: &u64) -> Result<(), IoError> {
        self.events.borrow_mut().push("payload_sync");
        if self.fault == Fault::PayloadSync {
            return Err(failure());
        }
        Ok(())
    }

    fn verify(&self, _: &u64) -> Result<(), IoError> {
        self.events.borrow_mut().push("verify");
        if self.fault == Fault::Verify {
            return Err(failure());
        }
        Ok(())
    }

    fn publish(&self, _: &u64) -> Result<(), PublishFailure> {
        self.events.borrow_mut().push("publish");
        match self.fault {
            Fault::Collision => Err(PublishFailure::Collision(IoError::Io(
                ErrorKind::AlreadyExists,
            ))),
            Fault::Unknown => Err(PublishFailure::Uncertain(failure())),
            Fault::RequestCancelAfterPublication => {
                self.cancelled.set(true);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn cleanup(&self, _: &u64) -> Result<(), IoError> {
        self.events.borrow_mut().push("cleanup");
        if matches!(
            self.fault,
            Fault::Cleanup | Fault::CleanupAndBarrier | Fault::WriteAndCleanup
        ) {
            return Err(failure());
        }
        Ok(())
    }

    fn sync_namespace(&self, stage: &u64) -> Result<(), IoError> {
        assert_eq!(*stage, 71);

        if self.guarantee == PublicationGuarantee::CapturedParentBarrier {
            return self.sync_parent();
        }

        self.events.borrow_mut().push("renamed_file_sync");
        if matches!(self.fault, Fault::ParentSync | Fault::CleanupAndBarrier) {
            return Err(failure());
        }
        Ok(())
    }

    fn sync_parent(&self) -> Result<(), IoError> {
        self.events.borrow_mut().push("parent_sync");
        if matches!(self.fault, Fault::ParentSync | Fault::CleanupAndBarrier) {
            return Err(failure());
        }
        Ok(())
    }
}

fn model<'a>(
    fault: Fault,
    events: &'a RefCell<Vec<&'static str>>,
    cancelled: &'a Cell<bool>,
) -> Model<'a> {
    Model {
        fault,
        events,
        cancelled,
        identity: ParentIdentity {
            #[cfg(not(windows))]
            device: 1,
            #[cfg(not(windows))]
            inode: 2,
            #[cfg(windows)]
            volume: 1,
            #[cfg(windows)]
            file_id: [2; 16],
        },
        guarantee: PublicationGuarantee::CapturedParentBarrier,
    }
}

#[test]
fn publication_fault_states_preserve_known_visibility_and_terminal_barrier() {
    for fault in [
        Fault::None,
        Fault::Create,
        Fault::CreateUnbound,
        Fault::Write,
        Fault::WriteAndCleanup,
        Fault::PayloadSync,
        Fault::Verify,
        Fault::Collision,
        Fault::Unknown,
        Fault::ParentSync,
        Fault::Cleanup,
        Fault::CleanupAndBarrier,
        Fault::RequestCancelAfterPublication,
    ] {
        let events = RefCell::new(Vec::new());
        let cancelled = Cell::new(false);
        let mut budget = ResourceBudget::new(4, 0, 100);
        let result = protocol::publish(
            model(fault, &events, &cancelled),
            b"COPY",
            &mut budget,
            &|| cancelled.get(),
        );
        let prefix = ["create", "write", "payload_sync", "verify", "publish"];
        match fault {
            Fault::None | Fault::RequestCancelAfterPublication => {
                assert!(result.is_ok());
                assert_eq!(
                    *events.borrow(),
                    [prefix.as_slice(), &["cleanup", "parent_sync"]].concat()
                );
            }
            Fault::Create | Fault::CreateUnbound => {
                assert!(matches!(result, Err(PublicationError::NotPublished {
                    temporary_may_remain, cleanup: None, ..
                }) if temporary_may_remain == (fault == Fault::CreateUnbound)));
                assert_eq!(*events.borrow(), ["create"]);
            }
            Fault::Write | Fault::WriteAndCleanup => {
                assert!(matches!(result, Err(PublicationError::NotPublished {
                    cleanup, temporary_may_remain, ..
                }) if cleanup.is_some() == (fault == Fault::WriteAndCleanup)
                    && temporary_may_remain == (fault == Fault::WriteAndCleanup)));
                assert_eq!(*events.borrow(), ["create", "write", "cleanup"]);
            }
            Fault::PayloadSync => {
                assert!(matches!(result, Err(PublicationError::NotPublished { .. })));
                assert_eq!(
                    *events.borrow(),
                    ["create", "write", "payload_sync", "cleanup"]
                );
            }
            Fault::Verify => {
                assert!(matches!(result, Err(PublicationError::NotPublished { .. })));
                assert_eq!(
                    *events.borrow(),
                    ["create", "write", "payload_sync", "verify", "cleanup"]
                );
            }
            Fault::Collision => {
                assert!(matches!(result, Err(PublicationError::NotPublished { .. })));
                assert_eq!(*events.borrow(), [prefix.as_slice(), &["cleanup"]].concat());
            }
            Fault::Unknown => {
                assert!(matches!(
                    result,
                    Err(PublicationError::VisibilityUncertain { .. })
                ));
                assert_eq!(*events.borrow(), prefix);
            }
            Fault::ParentSync => {
                assert!(matches!(
                    result,
                    Err(PublicationError::PublishedUnconfirmed { .. })
                ));
                assert_eq!(
                    *events.borrow(),
                    [prefix.as_slice(), &["cleanup", "parent_sync"]].concat()
                );
            }
            Fault::Cleanup | Fault::CleanupAndBarrier => {
                assert!(
                    matches!(result, Err(PublicationError::PublishedCleanupFailure {
                    namespace_synced, barrier, ..
                }) if namespace_synced == (fault == Fault::Cleanup)
                    && barrier.is_some() == (fault == Fault::CleanupAndBarrier))
                );
                assert_eq!(
                    *events.borrow(),
                    [prefix.as_slice(), &["cleanup", "parent_sync"]].concat()
                );
            }
        }
        assert_eq!(budget.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn publication_reserves_cleanup_and_terminal_work_before_visibility() {
    for (steps, expected) in [
        (8, Vec::new()),
        (
            16,
            vec!["create", "write", "payload_sync", "verify", "cleanup"],
        ),
        (
            17,
            vec![
                "create",
                "write",
                "payload_sync",
                "verify",
                "publish",
                "cleanup",
                "parent_sync",
            ],
        ),
    ] {
        let events = RefCell::new(Vec::new());
        let cancelled = Cell::new(false);
        let result = protocol::publish(
            model(Fault::None, &events, &cancelled),
            b"COPY",
            &mut ResourceBudget::new(4, 0, steps),
            &|| false,
        );
        assert_eq!(result.is_ok(), steps == 17);
        assert_eq!(*events.borrow(), expected);
    }

    let events = RefCell::new(Vec::new());
    let cancelled = Cell::new(false);
    let result = protocol::publish(
        model(Fault::None, &events, &cancelled),
        b"COPY",
        &mut ResourceBudget::new(3, 0, 100),
        &|| false,
    );
    assert!(matches!(result, Err(PublicationError::NotPublished { .. })));
    assert!(events.borrow().is_empty());
}

#[test]
fn publication_final_cancellation_cleans_without_publishing() {
    let events = RefCell::new(Vec::new());
    let cancelled = Cell::new(false);
    let polls = Cell::new(0);
    let result = protocol::publish(
        model(Fault::None, &events, &cancelled),
        b"COPY",
        &mut ResourceBudget::new(4, 0, 100),
        &|| {
            let poll = polls.get() + 1;
            polls.set(poll);
            poll == 4
        },
    );
    assert!(matches!(
        result,
        Err(PublicationError::NotPublished {
            cause: IoError::Cancelled,
            ..
        })
    ));
    assert_eq!(
        *events.borrow(),
        ["create", "write", "payload_sync", "verify", "cleanup"]
    );
}

#[cfg(unix)]
#[path = "tests/filesystem.rs"]
mod filesystem;

#[cfg(not(any(unix, windows)))]
#[test]
fn required_output_unsupported_platform_fails_before_effects() {
    use super::RequiredOutput;
    use std::path::Path;

    assert!(matches!(
        RequiredOutput::capture(
            Path::new("result"),
            &mut ResourceBudget::new(100, 0, 100),
            &|| false,
        ),
        Err(PublicationError::UnsupportedDurability)
    ));
}

#[path = "tests/windows_protocol.rs"]
mod windows_protocol;

use super::windows_names;

#[path = "tests/windows_names.rs"]
mod windows_name_tests;

#[cfg(windows)]
#[path = "tests/windows_filesystem.rs"]
mod windows_filesystem;
