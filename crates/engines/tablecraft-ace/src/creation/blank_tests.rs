//! Explicit blank creation is independent of the nonempty table-list contract.
use super::{CreateDatabaseError, create_ace16_database_tables, create_blank_ace16_database};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 100_000, 16_000_000)
}

#[test]
fn blank_creator_matches_independent_system_only_image_without_loosening_table_lists() {
    let created = create_blank_ace16_database(&mut budget(), &|| false).unwrap();
    assert_eq!(created.as_bytes(), canonical_blank_creation::source());
    assert_eq!(
        (
            created.table_count(),
            created.column_count(),
            created.row_count()
        ),
        (0, 0, 0),
    );
    assert!(matches!(
        create_ace16_database_tables(&[], &mut budget(), &|| false),
        Err(CreateDatabaseError::TableLimit),
    ));
}

#[test]
fn blank_creation_pays_system_resources_and_can_cancel_before_and_after_serialization() {
    let mut measured = budget();
    let polls = Cell::new(0_usize);
    create_blank_ace16_database(&mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let baseline = budget();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| baseline.remaining(resource) - measured.remaining(resource));

    for (index, resource) in resources.into_iter().enumerate() {
        assert!(
            used[index] > 0,
            "system structures must consume {resource:?}"
        );
        let mut pools = [16 * 1024 * 1024, 100_000, 16_000_000];
        pools[index] = used[index] - 1;
        let mut limited = ResourceBudget::new(pools[0], pools[1], pools[2]);
        assert!(matches!(
            create_blank_ace16_database(&mut limited, &|| false),
            Err(CreateDatabaseError::Inspect(InspectError::Limit(error)))
                if error.resource == resource,
        ));

        pools[index] = used[index];
        let mut exact = ResourceBudget::new(pools[0], pools[1], pools[2]);
        create_blank_ace16_database(&mut exact, &|| false).unwrap();
        assert_eq!(exact.remaining(resource), 0);
    }

    let mut untouched = budget();
    assert!(matches!(
        create_blank_ace16_database(&mut untouched, &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled)),
    ));
    assert_eq!(
        untouched.remaining(Resource::Bytes),
        baseline.remaining(Resource::Bytes)
    );

    let observed = Cell::new(0_usize);
    assert!(matches!(
        create_blank_ace16_database(&mut budget(), &|| {
            observed.set(observed.get() + 1);
            observed.get() == polls.get()
        }),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled)),
    ));
}
