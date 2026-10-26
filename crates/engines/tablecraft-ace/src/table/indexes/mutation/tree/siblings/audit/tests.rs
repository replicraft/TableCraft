use super::audit_siblings;
use crate::{
    EditError,
    table::indexes::mutation::{
        retirement::RetiredPage,
        tree::siblings::{Level, Node, SiblingAudit},
    },
};
use tablecraft_core::limits::ResourceBudget;

fn captured(budget: &mut ResourceBudget) -> SiblingAudit {
    // Explicit independent expected state: root1 above reciprocal nodes2,3.
    // Charge every retained descriptor just as captured production state does.
    budget
        .charge_allocation(2, size_of::<Level>() as u64)
        .unwrap();
    let mut levels = Vec::new();
    levels.try_reserve_exact(2).unwrap();
    for entries in [vec![(2, 0, 3), (3, 2, 0)], vec![(1, 0, 0)]] {
        budget
            .charge_allocation(entries.len() as u64, size_of::<Node>() as u64)
            .unwrap();
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(entries.len()).unwrap();
        for (number, previous, next) in entries {
            nodes.push(Node {
                number,
                previous,
                next,
            });
        }
        let linked = nodes.len() == 2;
        levels.push(Level { nodes, linked });
    }
    budget
        .charge_allocation(5, size_of::<u32>() as u64)
        .unwrap();
    let mut pages = Vec::new();
    pages.try_reserve_exact(5).unwrap();
    pages.extend([1, 2, 3, 4, 5]);
    SiblingAudit {
        root: 1,
        physical: 0,
        levels,
        pages,
        empty_root: false,
    }
}

fn put32(bytes: &mut [u8], number: usize, field: usize, value: u32) {
    let offset = number * 4096 + field;
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn retired(numbers: &[u32]) -> Vec<RetiredPage> {
    numbers
        .iter()
        .map(|&number| RetiredPage {
            number,
            physical: 0,
            usage: 99,
            root: 1,
        })
        .collect()
}

#[test]
fn independent_after_audit_rejects_dangling_links_header_changes_and_incomplete_empty_retirement() {
    let mut budget = ResourceBudget::new(10_000_000, 100_000, 10_000_000);
    let mut state = captured(&mut budget);
    let mut before = vec![0; 6 * 4096];
    for (number, kind, height) in [(1, 3, 2), (2, 3, 1), (3, 3, 1), (4, 4, 0), (5, 4, 0)] {
        before[number * 4096] = kind;
        before[number * 4096 + 1] = 1;
        before[number * 4096 + 26] = height;
        put32(&mut before, number, 4, 42);
    }
    put32(&mut before, 2, 16, 3);
    put32(&mut before, 3, 12, 2);
    let removed = retired(&[2, 4]);
    let mut valid = before.clone();
    valid[2 * 4096..2 * 4096 + 4].fill(0);
    valid[4 * 4096..4 * 4096 + 4].fill(0);
    put32(&mut valid, 3, 12, 0);
    assert_eq!(
        audit_siblings(
            &before,
            &valid,
            std::slice::from_ref(&state),
            &removed,
            &mut budget,
            &|| false
        ),
        Ok(())
    );
    for (number, field, value) in [
        (3, 12, 2),
        (3, 16, 2),
        (1, 0, 0x0104),
        (3, 26, 0),
        (3, 4, 43),
    ] {
        let mut forged = valid.clone();
        put32(&mut forged, number, field, value);
        assert_eq!(
            audit_siblings(
                &before,
                &forged,
                std::slice::from_ref(&state),
                &removed,
                &mut budget,
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
    state.empty_root = true;
    let removed = retired(&[2, 3, 4, 5]);
    let mut empty = before.clone();
    for number in [2, 3, 4, 5] {
        empty[number * 4096..number * 4096 + 4].fill(0);
    }
    empty[4096] = 4;
    empty[4096 + 26] = 0;
    empty[4096 + 2..4096 + 4].copy_from_slice(&3616_u16.to_le_bytes());
    assert_eq!(
        audit_siblings(
            &before,
            &empty,
            std::slice::from_ref(&state),
            &removed,
            &mut budget,
            &|| false
        ),
        Ok(())
    );
    for (field, value) in [(12, 2), (16, 3), (20, 4), (26, 2), (0, 0x0103)] {
        let mut forged = empty.clone();
        put32(&mut forged, 1, field, value);
        assert_eq!(
            audit_siblings(
                &before,
                &forged,
                std::slice::from_ref(&state),
                &removed,
                &mut budget,
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
    assert_eq!(
        audit_siblings(
            &before,
            &empty,
            &[state],
            &retired(&[2, 3, 5]),
            &mut budget,
            &|| false
        ),
        Err(EditError::Preservation)
    );
}
