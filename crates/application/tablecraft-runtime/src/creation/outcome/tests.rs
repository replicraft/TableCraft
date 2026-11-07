use super::*;
use std::io::ErrorKind;

#[test]
fn publication_failures_keep_exact_primary_and_secondary_facts() {
    let cases = [
        (
            PublicationError::NotPublished {
                cause: IoError::Cancelled,
                cleanup: Some(IoError::Io(ErrorKind::PermissionDenied)),
                temporary_may_remain: true,
            },
            CreationPublication::NotPublished {
                temporary_may_remain: true,
            },
        ),
        (
            PublicationError::VisibilityUncertain {
                cause: IoError::Io(ErrorKind::Interrupted),
            },
            CreationPublication::VisibilityUncertain,
        ),
        (
            PublicationError::PublishedUnconfirmed {
                cause: IoError::Io(ErrorKind::Other),
            },
            CreationPublication::PublishedUnconfirmed,
        ),
        (
            PublicationError::PublishedCleanupFailure {
                cause: IoError::Io(ErrorKind::PermissionDenied),
                namespace_synced: false,
                barrier: Some(IoError::Io(ErrorKind::Other)),
            },
            CreationPublication::PublishedCleanupFailure {
                namespace_synced: false,
            },
        ),
        (
            PublicationError::PublishedCleanupFailure {
                cause: IoError::Io(ErrorKind::PermissionDenied),
                namespace_synced: true,
                barrier: None,
            },
            CreationPublication::PublishedCleanupFailure {
                namespace_synced: true,
            },
        ),
        (
            PublicationError::UnsupportedDurability,
            CreationPublication::NotPublished {
                temporary_may_remain: false,
            },
        ),
    ];

    for (native, publication) in cases {
        let failure =
            CreationFailure::native(CreationPhase::Publish, CreationError::Publication(native));
        assert_eq!(failure.phase, Some(CreationPhase::Publish));
        assert_eq!(failure.publication, publication);
        let retained = failure
            .source()
            .unwrap()
            .downcast_ref::<CreationError>()
            .unwrap();
        let CreationError::Publication(retained) = retained else {
            panic!("publication error must remain typed");
        };
        match retained {
            PublicationError::NotPublished {
                cause,
                cleanup,
                temporary_may_remain,
            } => {
                assert_eq!(cause, &IoError::Cancelled);
                assert_eq!(cleanup, &Some(IoError::Io(ErrorKind::PermissionDenied)));
                assert!(*temporary_may_remain);
                assert!(failure.is_cancelled());
            }
            PublicationError::PublishedCleanupFailure {
                cause,
                namespace_synced,
                barrier,
            } => {
                assert_eq!(cause, &IoError::Io(ErrorKind::PermissionDenied));
                if *namespace_synced {
                    assert_eq!(barrier, &None);
                } else {
                    assert_eq!(barrier, &Some(IoError::Io(ErrorKind::Other)));
                }
                assert!(!failure.is_cancelled());
            }
            PublicationError::VisibilityUncertain { cause } => {
                assert_eq!(cause, &IoError::Io(ErrorKind::Interrupted));
            }
            PublicationError::PublishedUnconfirmed { cause } => {
                assert_eq!(cause, &IoError::Io(ErrorKind::Other));
            }
            PublicationError::UnsupportedDurability => {
                assert!(failure.unsupported_durability());
            }
        }
    }
}
