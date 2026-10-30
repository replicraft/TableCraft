use super::protocol::{self, Backend, CreateFailure, PublishFailure};
use super::{ParentIdentity, PublicationError};
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
}

fn failure() -> IoError {
    IoError::Io(ErrorKind::Other)
}

impl Backend for Model<'_> {
    type Stage = ();

    fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    fn create(&self, _: &mut ResourceBudget, _: &dyn Fn() -> bool) -> Result<(), CreateFailure> {
        self.events.borrow_mut().push("create");
        if matches!(self.fault, Fault::Create | Fault::CreateUnbound) {
            return Err(CreateFailure {
                cause: failure(),
                temporary_may_remain: self.fault == Fault::CreateUnbound,
            });
        }
        Ok(())
    }

    fn write(
        &self,
        _: &mut (),
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

    fn sync_payload(&self, _: &()) -> Result<(), IoError> {
        self.events.borrow_mut().push("payload_sync");
        if self.fault == Fault::PayloadSync {
            return Err(failure());
        }
        Ok(())
    }

    fn verify(&self, _: &()) -> Result<(), IoError> {
        self.events.borrow_mut().push("verify");
        if self.fault == Fault::Verify {
            return Err(failure());
        }
        Ok(())
    }

    fn publish(&self, _: &()) -> Result<(), PublishFailure> {
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

    fn cleanup(&self, _: &()) -> Result<(), IoError> {
        self.events.borrow_mut().push("cleanup");
        if matches!(
            self.fault,
            Fault::Cleanup | Fault::CleanupAndBarrier | Fault::WriteAndCleanup
        ) {
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
            device: 1,
            inode: 2,
        },
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

#[cfg(not(unix))]
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
