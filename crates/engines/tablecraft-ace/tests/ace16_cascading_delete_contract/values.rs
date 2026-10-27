use super::model::*;
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, TableRowDeletes, WriteBlocker};

#[test]
fn independent_original_associations_expand_roots_and_exempt_null_children() {
    for code in [5, 6] {
        let model = database(code, true);
        before(&model);
        require_primary(&model, 1);
        for (selected, children, other) in [
            (vec![0], vec![0, 1], vec![]),
            (vec![1], vec![], vec![0]),
            (vec![2], vec![], vec![]),
            (vec![0, 1, 2], vec![0, 1], vec![0]),
        ] {
            let candidate = run(&model, &[(1, &selected)]).unwrap();
            verify(&model, &candidate, [&children, &selected, &other]);
        }
    }
}

#[test]
fn explicit_children_coalesce_with_generated_roots_without_reverse_cascades() {
    let model = database(6, true);
    before(&model);
    let root_only = run(&model, &[(1, &[0])]).unwrap();
    for seeds in [
        vec![(1, &[0][..]), (0, &[1][..])],
        vec![(0, &[1][..]), (1, &[0][..])],
    ] {
        let candidate = run(&model, &seeds).unwrap();
        verify(&model, &candidate, [&[0, 1], &[0], &[]]);
        assert_eq!(candidate.candidate_bytes(), root_only.candidate_bytes());
    }
    let child = run(&model, &[(0, &[0])]).unwrap();
    verify(&model, &child, [&[0], &[], &[]]);
    let null_child = run(&model, &[(0, &[2])]).unwrap();
    verify(&model, &null_child, [&[2], &[], &[]]);
}

#[test]
fn mixed_restrict_edges_keep_final_image_membership_and_old_api_boundaries() {
    let model = database(6, false);
    before(&model);
    let first = run(&model, &[(1, &[0])]).unwrap();
    verify(&model, &first, [&[0, 1], &[0], &[]]);

    assert_eq!(
        run(&model, &[(1, &[0, 1])]).err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::MissingParent
        )))
    );
    for seeds in [
        vec![(1, &[0, 1][..]), (2, &[0][..])],
        vec![(2, &[0][..]), (1, &[0, 1][..])],
    ] {
        let candidate = run(&model, &seeds).unwrap();
        verify(&model, &candidate, [&[0, 1], &[0, 1], &[0]]);
    }

    let roots = requests(&model, 1, &[0]);
    for empty in [false, true] {
        let deletes = if empty { &[][..] } else { roots.as_slice() };
        assert_eq!(
            capture(&model.bytes)
                .preview_ace16_deletes(table(1), deletes, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
        assert_eq!(
            capture(&model.bytes)
                .preview_ace16_batch_deletes(
                    &[TableRowDeletes::new(table(1), deletes)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
}
