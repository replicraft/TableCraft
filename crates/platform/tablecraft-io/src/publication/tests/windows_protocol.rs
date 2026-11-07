use super::{Fault, PublicationError, PublicationGuarantee, model, protocol};
use std::cell::{Cell, RefCell};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn write_through_receipt_retains_its_distinct_guarantee_and_stage_flush() {
    for fault in [Fault::None, Fault::RequestCancelAfterPublication] {
        let events = RefCell::new(Vec::new());
        let cancelled = Cell::new(false);
        let mut backend = model(fault, &events, &cancelled);
        backend.guarantee = PublicationGuarantee::NtfsWriteThroughRename;

        let receipt = protocol::publish(
            backend,
            b"COPY",
            &mut ResourceBudget::new(4, 0, 100),
            &|| cancelled.get(),
        )
        .unwrap();

        assert_eq!(
            receipt.guarantee(),
            PublicationGuarantee::NtfsWriteThroughRename
        );
        assert!(!receipt.namespace_synced());
        assert_eq!(
            *events.borrow(),
            [
                "create",
                "write",
                "payload_sync",
                "verify",
                "publish",
                "cleanup",
                "renamed_file_sync",
            ]
        );
    }
}

#[test]
fn write_through_terminal_failures_do_not_claim_a_parent_barrier() {
    for fault in [Fault::ParentSync, Fault::Cleanup, Fault::CleanupAndBarrier] {
        let events = RefCell::new(Vec::new());
        let cancelled = Cell::new(false);
        let mut backend = model(fault, &events, &cancelled);
        backend.guarantee = PublicationGuarantee::NtfsWriteThroughRename;

        let result = protocol::publish(
            backend,
            b"COPY",
            &mut ResourceBudget::new(4, 0, 100),
            &|| false,
        );

        match fault {
            Fault::ParentSync => {
                assert!(matches!(
                    result,
                    Err(PublicationError::PublishedUnconfirmed { .. })
                ));
            }
            Fault::Cleanup | Fault::CleanupAndBarrier => {
                assert!(matches!(
                    result,
                    Err(PublicationError::PublishedCleanupFailure {
                        namespace_synced: false,
                        barrier,
                        ..
                    }) if barrier.is_some() == (fault == Fault::CleanupAndBarrier)
                ));
            }
            _ => unreachable!(),
        }

        assert_eq!(events.borrow().last(), Some(&"renamed_file_sync"));
        assert!(!events.borrow().contains(&"parent_sync"));
    }
}
