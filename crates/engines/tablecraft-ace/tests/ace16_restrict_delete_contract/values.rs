use super::model::*;
use tablecraft_ace::{ConstraintViolation, EditError, InspectError};

fn missing_parent() -> EditError {
    EditError::Inspect(InspectError::Constraint(ConstraintViolation::MissingParent))
}

#[test]
fn child_null_and_unreferenced_parent_deletions_keep_both_known_update_actions_inert() {
    for code in [5, 6] {
        for update_cascade in [false, true] {
            let model = database(code, false, false, update_cascade);
            for (target, deleted) in [
                (0, vec![]),
                (0, vec![0]),
                (0, vec![2]),
                (0, vec![0, 1, 2, 3]),
                (1, vec![1, 2]),
            ] {
                let output = candidate(&model, target, &deleted).unwrap();
                verify(&model, &output, target, &deleted);
            }
            assert_eq!(candidate(&model, 1, &[0]).err(), Some(missing_parent()));
        }
    }
}

#[test]
fn all_null_composites_are_exempt_but_partial_null_children_still_restrict() {
    for code in [5, 6] {
        let model = database(code, true, false, false);
        for deleted in [vec![2], vec![0, 3], vec![0, 1, 2, 3]] {
            verify(
                &model,
                &candidate(&model, 0, &deleted).unwrap(),
                0,
                &deleted,
            );
        }
        for row in [0, 1] {
            assert_eq!(candidate(&model, 1, &[row]).err(), Some(missing_parent()));
        }
        verify(&model, &candidate(&model, 1, &[2]).unwrap(), 1, &[2]);
    }
}

#[test]
fn a_second_incoming_edge_blocks_an_otherwise_unreferenced_parent_atomically() {
    let single = database(6, false, false, false);
    verify(&single, &candidate(&single, 1, &[1]).unwrap(), 1, &[1]);
    let multiple = database(6, false, true, false);
    assert_eq!(
        candidate(&multiple, 1, &[1, 2]).err(),
        Some(missing_parent())
    );
    verify(&multiple, &candidate(&multiple, 2, &[0]).unwrap(), 2, &[0]);
    verify(&multiple, &candidate(&multiple, 1, &[2]).unwrap(), 1, &[2]);
    assert_eq!(multiple.bytes, database(6, false, true, false).bytes);
}
