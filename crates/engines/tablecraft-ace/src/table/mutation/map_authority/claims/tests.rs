//! Isolate footprint authority; these claims do not pretend to be native pages.
use super::{Census, Claim, Owner};
use crate::{EditError, WriteBlocker, format::allocation::MapStorage};
use tablecraft_core::limits::ResourceBudget;

fn claim(owner: Owner, reference: u32, start: usize) -> Claim {
    Claim {
        owner,
        reference,
        storage: MapStorage {
            record: start..start + 69,
            pages: vec![reference >> 8],
        },
    }
}

#[test]
fn different_references_do_not_authorize_overlapping_map_record_bytes() {
    let selected = [
        Owner::GlobalFree,
        Owner::OrdinaryOwned { definition: 10 },
        Owner::OrdinaryCandidate { definition: 10 },
        Owner::ColumnOwned {
            definition: 10,
            column: 1,
        },
        Owner::ColumnCandidate {
            definition: 10,
            column: 1,
        },
    ];
    let base = 11 * 4096;
    let mut census = Census {
        claims: vec![
            claim(Owner::GlobalFree, 1 << 8, 4096 + 4000),
            claim(
                Owner::OrdinaryOwned { definition: 10 },
                11 << 8,
                base + 4000,
            ),
            claim(
                Owner::OrdinaryCandidate { definition: 10 },
                (11 << 8) | 1,
                base + 3900,
            ),
            claim(
                Owner::ColumnOwned {
                    definition: 10,
                    column: 1,
                },
                (11 << 8) | 2,
                base + 3800,
            ),
            claim(
                Owner::ColumnCandidate {
                    definition: 10,
                    column: 1,
                },
                (11 << 8) | 3,
                base + 3700,
            ),
            claim(
                Owner::ColumnOwned {
                    definition: 15,
                    column: 99,
                },
                (11 << 8) | 6,
                base + 3820,
            ),
        ],
    };
    assert_eq!(
        census
            .require_exclusive(
                &selected,
                &mut ResourceBudget::new(10000, 100, 10000),
                &|| { false }
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    census.claims[5].storage.record = base + 3600..base + 3669;
    census
        .require_exclusive(
            &selected,
            &mut ResourceBudget::new(10000, 100, 10000),
            &|| false,
        )
        .unwrap();

    for owner in [
        Owner::IndexOwned {
            definition: 15,
            physical: 0,
        },
        Owner::ColumnOwned {
            definition: 15,
            column: 99,
        },
    ] {
        census.claims[5].owner = owner;
        census.claims[5].reference = (11 << 8) | 2;
        assert_eq!(
            census
                .require_exclusive(
                    &selected,
                    &mut ResourceBudget::new(10000, 100, 10000),
                    &|| { false },
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn every_selected_owner_must_exist_once_without_a_fixed_role_count() {
    let owner = Owner::IndexOwned {
        definition: 10,
        physical: 2,
    };
    let census = Census {
        claims: vec![claim(owner, 11 << 8, 11 * 4096 + 3900)],
    };
    let run = |selected: &[Owner]| {
        census.require_exclusive(
            selected,
            &mut ResourceBudget::new(10000, 100, 10000),
            &|| false,
        )
    };
    run(&[owner]).unwrap();
    for selected in [vec![owner, owner], vec![Owner::GlobalFree]] {
        assert_eq!(
            run(&selected).err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn bitmap_storage_cannot_alias_another_container_or_bitmap() {
    let owner = Owner::GlobalFree;
    let other = Owner::IndexOwned {
        definition: 10,
        physical: 0,
    };
    let mut census = Census {
        claims: vec![
            claim(owner, 1 << 8, 4096 + 3900),
            claim(other, 11 << 8, 11 * 4096 + 3900),
        ],
    };
    for (selected_pages, foreign_pages) in [
        (vec![1, 11], vec![11]),
        (vec![1], vec![11, 1]),
        (vec![1, 20], vec![11, 20]),
    ] {
        census.claims[0].storage.pages = selected_pages;
        census.claims[1].storage.pages = foreign_pages;
        assert_eq!(
            census
                .require_exclusive(
                    &[owner],
                    &mut ResourceBudget::new(10000, 100, 10000),
                    &|| false,
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}

#[test]
fn owner_selection_checks_cancellation_and_its_work_allowance() {
    use crate::InspectError;

    let census = Census {
        claims: vec![claim(Owner::GlobalFree, 1 << 8, 4096 + 3900)],
    };
    let selected = [Owner::GlobalFree];
    assert_eq!(
        census
            .require_exclusive(
                &selected,
                &mut ResourceBudget::new(10000, 100, 10000),
                &|| true,
            )
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled))
    );
    assert!(matches!(
        census.require_exclusive(&selected, &mut ResourceBudget::new(10000, 100, 0), &|| {
            false
        },),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    census
        .require_exclusive(
            &selected,
            &mut ResourceBudget::new(10000, 100, 10000),
            &|| false,
        )
        .unwrap();
}
