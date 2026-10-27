//! Isolate footprint authority; these claims do not pretend to be native pages.
use super::{Census, Claim, Role};
use crate::{EditError, WriteBlocker, format::allocation::MapStorage};
use tablecraft_core::limits::ResourceBudget;

fn claim(definition: u32, role: Role, reference: u32, start: usize) -> Claim {
    Claim {
        definition,
        role,
        reference,
        storage: MapStorage {
            record: start..start + 69,
            pages: vec![reference >> 8],
        },
    }
}

#[test]
fn different_references_do_not_authorize_overlapping_map_record_bytes() {
    let base = 11 * 4096;
    let mut census = Census {
        claims: vec![
            claim(0, Role::GlobalFree, 1 << 8, 4096 + 4000),
            claim(10, Role::OrdinaryOwned, 11 << 8, base + 4000),
            claim(10, Role::OrdinaryCandidate, (11 << 8) | 1, base + 3900),
            claim(10, Role::ColumnOwned(1), (11 << 8) | 2, base + 3800),
            claim(10, Role::ColumnCandidate(1), (11 << 8) | 3, base + 3700),
            claim(15, Role::ColumnOwned(99), (11 << 8) | 6, base + 3820),
        ],
    };
    assert_eq!(
        census
            .require_exclusive(10, 1, &mut ResourceBudget::new(10000, 100, 10000), &|| {
                false
            })
            .err(),
        Some(EditError::Unsupported(WriteBlocker::RowLayout))
    );
    census.claims[5].storage.record = base + 3600..base + 3669;
    census
        .require_exclusive(10, 1, &mut ResourceBudget::new(10000, 100, 10000), &|| {
            false
        })
        .unwrap();

    for role in [Role::IndexOwned(0), Role::ColumnOwned(99)] {
        census.claims[5].role = role;
        census.claims[5].reference = (11 << 8) | 2;
        assert_eq!(
            census
                .require_exclusive(10, 1, &mut ResourceBudget::new(10000, 100, 10000), &|| {
                    false
                },)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::RowLayout))
        );
    }
}
