//! DELETE-generated NULLs traverse only declared ON UPDATE CASCADE edges.
#[path = "update_closure/model.rs"]
mod literal;

use super::model;
use literal::Shape;
use std::cell::Cell;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, RowDelete, TableRowDeletes,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};

fn transition(code: u8, shape: Shape) -> (model::Model, model::Expected) {
    let source = literal::source(code, shape);
    let expected = literal::expected(&source, [&[], &[1], &[]], [&[1], &[], &[0]]);
    (source, expected)
}

#[test]
fn qualify_update_closure_sources_and_full_expected_images_before_preview() {
    for code in [5, 6] {
        for shape in [Shape::Chain, Shape::Tuple, Shape::Diamond, Shape::Cycle] {
            let (source, expected) = transition(code, shape);
            eprintln!("qualify header {code}, shape {shape:?}");
            model::before(&source);
            model::qualify(&expected);
            assert_eq!(expected.removed, 1);
            assert_eq!(expected.rows[0][1][1], Value::Null);
            assert_eq!(expected.rows[2][0][1], Value::Null);
            if source.tuple {
                assert_eq!(expected.rows[0][1][2], Value::Null);
                assert_eq!(expected.rows[2][0][2], Value::Null);
            }
        }
        for shape in [Shape::Restrict, Shape::Required] {
            eprintln!("qualify header {code}, shape {shape:?}");
            model::before(&literal::source(code, shape));
        }
        let restricted = literal::source(code, Shape::Restrict);
        model::qualify(&literal::expected(
            &restricted,
            [&[], &[1], &[0]],
            [&[1], &[], &[]],
        ));
        let source = literal::source(code, Shape::Chain);
        for (deleted, nulls) in [
            ([&[][..], &[0][..], &[][..]], [&[0][..], &[][..], &[][..]]),
            ([&[][..], &[1][..], &[0][..]], [&[1][..], &[][..], &[][..]]),
            ([&[1][..], &[1][..], &[0][..]], [&[][..], &[][..], &[][..]]),
        ] {
            model::qualify(&literal::expected(&source, deleted, nulls));
        }
    }
}

#[test]
fn generated_nulls_propagate_through_whole_tuple_chain_and_convergent_cycles() {
    for code in [5, 6] {
        for shape in [Shape::Chain, Shape::Tuple, Shape::Diamond, Shape::Cycle] {
            let (source, expected) = transition(code, shape);
            model::before(&source);
            model::qualify(&expected);
            let candidate = model::run(&source, &[(1, &[1])]).unwrap();
            model::verify(&source, &expected, &candidate);
            // SET NULL and repeated diamond/cycle demands never count as deletes.
            assert_eq!(candidate.deleted_rows(), 1);
        }
    }
}

#[test]
fn no_matching_descendant_and_deleted_endpoints_do_not_generate_extra_nulls() {
    for code in [5, 6] {
        let source = literal::source(code, Shape::Chain);
        model::before(&source);
        for (seeds, deleted, nulls) in [
            (
                vec![(1, vec![0])],
                [&[][..], &[0][..], &[][..]],
                [&[0][..], &[][..], &[][..]],
            ),
            (
                vec![(1, vec![1]), (2, vec![0])],
                [&[][..], &[1][..], &[0][..]],
                [&[1][..], &[][..], &[][..]],
            ),
            (
                vec![(1, vec![1]), (0, vec![1]), (2, vec![0])],
                [&[1][..], &[1][..], &[0][..]],
                [&[][..], &[][..], &[][..]],
            ),
        ] {
            let expected = literal::expected(&source, deleted, nulls);
            model::qualify(&expected);
            let seeds = seeds
                .iter()
                .map(|(table, rows)| (*table, rows.as_slice()))
                .collect::<Vec<_>>();
            let candidate = model::run(&source, &seeds).unwrap();
            model::verify(&source, &expected, &candidate);
        }
    }
}

#[test]
fn downstream_restrict_and_required_rules_refuse_the_entire_candidate() {
    for code in [5, 6] {
        let source = literal::source(code, Shape::Restrict);
        model::before(&source);
        assert_eq!(
            model::run(&source, &[(1, &[1])]).err(),
            Some(EditError::ReferentialAction)
        );
        let expected = literal::expected(&source, [&[], &[0], &[]], [&[0], &[], &[]]);
        model::qualify(&expected);
        let candidate = model::run(&source, &[(1, &[0])]).unwrap();
        model::verify(&source, &expected, &candidate);

        // A removed downstream endpoint cannot restrict the surviving parent's
        // NULL assignment. Both actual deletions remain in the delete count.
        let expected = literal::expected(&source, [&[], &[1], &[0]], [&[1], &[], &[]]);
        model::qualify(&expected);
        let candidate = model::run(&source, &[(1, &[1]), (2, &[0])]).unwrap();
        model::verify(&source, &expected, &candidate);

        let source = literal::source(code, Shape::Required);
        model::before(&source);
        assert_eq!(
            model::run(&source, &[(1, &[1])]).err(),
            Some(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::NullKey,
            ))),
        );
    }
}

#[test]
fn update_closure_stale_inputs_limits_and_terminal_cancellation_publish_nothing() {
    let (source, expected) = transition(6, Shape::Diamond);
    model::before(&source);
    model::qualify(&expected);
    let requests = model::requests(&source, 1, &[1]);
    let groups = [TableRowDeletes::new("Parents", &requests)];
    for mut limited in [
        ResourceBudget::new(0, 100_000, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 40_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 100_000, 0),
    ] {
        assert!(
            model::capture(&source.bytes)
                .preview_ace16_cascading_deletes(&groups, &mut limited, &|| false)
                .is_err()
        );
    }
    let mut wrong = source.records[1][1].clone();
    wrong[2] ^= 1;
    let stale = [RowDelete::new(1, wrong)];
    let stale_groups = [TableRowDeletes::new("Parents", &stale)];
    assert_eq!(
        model::capture(&source.bytes)
            .preview_ace16_cascading_deletes(&stale_groups, &mut model::budget(), &|| false)
            .err(),
        Some(EditError::Conflict),
    );
    let polls = Cell::new(0);
    let candidate = model::capture(&source.bytes)
        .preview_ace16_cascading_deletes(&groups, &mut model::budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    model::verify(&source, &expected, &candidate);
    let terminal = polls.get();
    assert!(terminal > 1);
    let polls = Cell::new(0);
    assert!(
        model::capture(&source.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut model::budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() >= terminal
            })
            .is_err()
    );
    assert_eq!(polls.get(), terminal);
}

#[test]
fn qualify_restrict_diamond_sources_and_membership_valid_expected_images() {
    for code in [5, 6] {
        let source = literal::source(code, Shape::RestrictDiamond);
        model::before(&source);
        // This final state has valid membership, despite the declared RESTRICT
        // action forbidding its surviving original association from changing.
        let surviving = literal::expected(&source, [&[], &[1], &[]], [&[1], &[], &[0]]);
        model::qualify(&surviving);
        assert_eq!(surviving.rows[0][1][1], Value::Null);
        assert_eq!(surviving.rows[2][0][1], Value::Null);
        assert_eq!(surviving.removed, 1);

        let removed = literal::expected(&source, [&[], &[1], &[0]], [&[1], &[], &[]]);
        model::qualify(&removed);
        assert!(removed.rows[2].is_empty());
        assert_eq!(removed.removed, 2);
    }
}

#[test]
fn independently_nulled_surviving_child_does_not_bypass_update_restrict() {
    for code in [5, 6] {
        let source = literal::source(code, Shape::RestrictDiamond);
        model::before(&source);
        let membership_valid = literal::expected(&source, [&[], &[1], &[]], [&[1], &[], &[0]]);
        model::qualify(&membership_valid);
        assert_eq!(
            model::run(&source, &[(1, &[1])]).err(),
            Some(EditError::ReferentialAction),
        );

        // Removing the dependent itself suppresses its original UPDATE
        // association, while the other surviving row still receives SET NULL.
        let expected = literal::expected(&source, [&[], &[1], &[0]], [&[1], &[], &[]]);
        model::qualify(&expected);
        let candidate = model::run(&source, &[(1, &[1]), (2, &[0])]).unwrap();
        model::verify(&source, &expected, &candidate);
    }
}
