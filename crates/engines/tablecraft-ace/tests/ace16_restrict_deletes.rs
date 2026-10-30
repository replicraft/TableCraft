#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{ConstraintViolation, EditError, InspectError, read_table};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn rows(bytes: &[u8], table: &str) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        table,
        &mut model::budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}

#[test]
fn cross_table_child_and_unreferenced_parent_deletions_preserve_other_tables() {
    for code in [5, 6] {
        for update_cascade in [false, true] {
            let model = model::database(code, false, false, update_cascade);
            let child = model::candidate(&model, 0, &[0, 2]).unwrap();
            assert_eq!(child.deleted_rows(), 2);
            assert_eq!(
                rows(child.candidate_bytes(), "Items"),
                vec![
                    vec![Value::Int32(2), Value::Int32(10), Value::Int32(200)],
                    vec![Value::Int32(4), Value::Null, Value::Int32(400)],
                ]
            );
            assert_eq!(
                &child.candidate_bytes()[15 * 4096..],
                &model.bytes[15 * 4096..]
            );
            let parent = model::candidate(&model, 1, &[1]).unwrap();
            assert_eq!(
                rows(parent.candidate_bytes(), "Parents"),
                vec![
                    vec![Value::Int32(1), Value::Int32(10), Value::Int32(100)],
                    vec![Value::Int32(3), Value::Int32(30), Value::Int32(300)],
                ]
            );
            assert_eq!(
                &parent.candidate_bytes()[..15 * 4096],
                &model.bytes[..15 * 4096]
            );
            assert_eq!(
                &parent.candidate_bytes()[20 * 4096..],
                &model.bytes[20 * 4096..]
            );
        }
    }
}

#[test]
fn surviving_dependents_reject_parent_removal_without_inventing_delete_actions() {
    for code in [5, 6] {
        let model = model::database(code, false, false, true);
        let original = model.bytes.clone();
        for deleted in [vec![0], vec![0, 1, 2]] {
            assert_eq!(
                model::candidate(&model, 1, &deleted).err(),
                Some(EditError::Inspect(InspectError::Constraint(
                    ConstraintViolation::MissingParent
                )))
            );
            assert_eq!(model.bytes, original);
        }
        let children = model::candidate(&model, 0, &[0, 1, 2, 3]).unwrap();
        assert!(rows(children.candidate_bytes(), "Items").is_empty());
        assert_eq!(
            &children.candidate_bytes()[15 * 4096..],
            &original[15 * 4096..]
        );
        assert_eq!(
            model::candidate(&model, 0, &[]).unwrap().candidate_bytes(),
            original
        );
    }
}
