use super::model::*;
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, RowDelete, TableRowDeletes};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn literal_sources_and_expected_images_qualify_without_invoking_the_producer() {
    for code in [5, 6] {
        for composite in [false, true] {
            let model = source(code, composite, true);
            before(&model);
            let nulled: &[usize] = if composite { &[0, 3] } else { &[0, 1] };
            qualify(&expected(&model, [&[], &[0], &[]], nulled));
            qualify(&expected(
                &model,
                [&[0], &[0, 1], &[0]],
                if composite { &[3, 1] } else { &[1] },
            ));
        }
    }
}

#[test]
fn ignored_null_sources_and_expected_membership_qualify_before_mutation() {
    for composite in [false, true] {
        let model = with_ignored_child(source(6, composite, true));
        before(&model);
        let nulled: &[usize] = if composite { &[0, 3] } else { &[0, 1] };
        qualify(&expected(&model, [&[], &[0], &[]], nulled));
    }
}

#[test]
fn ignored_null_index_omission_preserves_child_rows_and_exact_table_counts() {
    for composite in [false, true] {
        let model = with_ignored_child(source(6, composite, true));
        before(&model);
        let nulled: &[usize] = if composite { &[0, 3] } else { &[0, 1] };
        let wanted = expected(&model, [&[], &[0], &[]], nulled);
        qualify(&wanted);
        let candidate = run(&model, &[(1, &[0])]).unwrap();
        verify(&model, &wanted, &candidate);
    }
}

#[test]
fn set_null_preserves_surviving_payloads_and_does_not_inflate_deleted_rows() {
    for code in [5, 6] {
        for composite in [false, true] {
            let model = source(code, composite, true);
            let nulled: &[usize] = if composite { &[0, 3] } else { &[0, 1] };
            let wanted = expected(&model, [&[], &[0], &[]], nulled);
            qualify(&wanted);
            let candidate = run(&model, &[(1, &[0])]).unwrap();
            verify(&model, &wanted, &candidate);
        }
    }
}

#[test]
fn explicit_child_deletion_and_cascade_share_the_null_key_index_transform() {
    for composite in [false, true] {
        let model = source(6, composite, true);
        let nulled: &[usize] = if composite { &[1, 3] } else { &[1] };
        let wanted = expected(&model, [&[0], &[0, 1], &[0]], nulled);
        qualify(&wanted);
        for seeds in [
            vec![(1, &[0, 1][..]), (0, &[0][..])],
            vec![(0, &[0][..]), (1, &[1, 0][..])],
        ] {
            let candidate = run(&model, &seeds).unwrap();
            verify(&model, &wanted, &candidate);
        }
    }
}

#[test]
fn restrict_dependents_validate_the_final_image_after_other_keys_are_nulled() {
    let model = source(6, false, false);
    before(&model);
    assert_eq!(
        run(&model, &[(1, &[0, 1])]).err(),
        Some(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::MissingParent,
        ))),
    );

    let wanted = expected(&model, [&[], &[0, 1], &[0]], &[0, 1]);
    qualify(&wanted);
    let candidate = run(&model, &[(1, &[0, 1]), (2, &[0])]).unwrap();
    verify(&model, &wanted, &candidate);
}

#[test]
fn empty_seeds_and_incoming_child_deletion_do_not_activate_parent_actions() {
    let model = source(6, false, true);
    for seeds in [vec![], vec![(1, &[][..])], vec![(0, &[][..])]] {
        let candidate = run(&model, &seeds).unwrap();
        assert_eq!(candidate.deleted_rows(), 0);
        assert_eq!(candidate.candidate_bytes(), model.bytes);
    }

    let wanted = expected(&model, [&[0], &[], &[]], &[]);
    qualify(&wanted);
    let candidate = run(&model, &[(0, &[0])]).unwrap();
    verify(&model, &wanted, &candidate);
}

#[test]
fn stale_explicit_children_are_checked_even_when_their_keys_would_be_nulled() {
    let model = source(6, false, true);
    let parents = requests(&model, 1, &[0]);
    let mut stale = model.records[0][0].clone();
    stale[2] ^= 1;
    let children = [RowDelete::new(0, stale)];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(
                &[
                    TableRowDeletes::new("Parents", &parents),
                    TableRowDeletes::new("Items", &children)
                ],
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Conflict),
    );
}

#[test]
fn cancellation_and_resource_exhaustion_never_publish_a_partial_candidate() {
    let model = source(6, true, true);
    let parents = requests(&model, 1, &[0]);
    let groups = [TableRowDeletes::new("Parents", &parents)];
    assert_eq!(
        capture(&model.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut budget(), &|| true)
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled)),
    );
    for mut limited in [
        ResourceBudget::new(0, 100_000, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            capture(&model.bytes)
                .preview_ace16_cascading_deletes(&groups, &mut limited, &|| false)
                .is_err()
        );
    }
}
