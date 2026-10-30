#![cfg(feature = "ace16-write-preview")]

use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "ace16_restrict_delete_contract/model.rs"]
mod model;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_ace::{EditError, RowDelete, TableRowDeletes, WriteBlocker, read_table};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const DATA_SLOT_START: usize = 14;
const PHYSICAL_FLAGS: usize = 46;
const PHYSICAL_BYTES: usize = 52;

fn cascade(code: u8) -> model::Model {
    let mut value = model::database(code, false, false, false);
    let parent = model::definition(1) * model::PAGE_BYTES;
    let physical = parent + value.logical_start - PHYSICAL_BYTES;
    value.bytes[physical + PHYSICAL_FLAGS] = 0x89;
    value.bytes[parent + value.logical_start + model::INDEX_KIND] = 1;
    for table in [0, 1] {
        let start = model::definition(table) * model::PAGE_BYTES + value.logical_start;
        for logical in 0..2 {
            let at = start + logical * model::LOGICAL_BYTES;
            if value.bytes[at + model::INDEX_KIND] == 2 {
                value.bytes[at + model::DELETE_ACTION] = 1;
            }
        }
    }
    model::relationships(&mut value, 0x1000);
    value
}

fn rows(bytes: &[u8], name: &str) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        name,
        &mut model::budget(),
        &|| false,
    )
    .unwrap()
    .into_parts()
    .2
}

#[test]
fn declared_cascade_removes_original_dependents_without_touching_null_children() {
    for code in [5, 6] {
        let value = cascade(code);
        let requests = model::requests(&value, 1, &[0]);
        let output = model::capture(&value.bytes)
            .preview_ace16_cascading_deletes(
                &[TableRowDeletes::new("Parents", &requests)],
                &mut model::budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(output.deleted_rows(), 3);
        assert_eq!(output.before_bytes(), value.bytes);
        assert_eq!(
            rows(output.candidate_bytes(), "Parents"),
            value.rows[1][1..]
        );
        assert_eq!(rows(output.candidate_bytes(), "Items"), value.rows[0][2..]);
        assert_eq!(rows(output.candidate_bytes(), "Other"), value.rows[2]);
        let unrelated = model::definition(2) * model::PAGE_BYTES;
        assert_eq!(
            &output.candidate_bytes()[unrelated..],
            &value.bytes[unrelated..]
        );
        for table in [0, 1] {
            for (row, record) in value.records[table].iter().enumerate() {
                let slot = model::data(table) * model::PAGE_BYTES
                    + DATA_SLOT_START
                    + row * size_of::<u16>();
                let word = u16::from_le_bytes(value.bytes[slot..slot + 2].try_into().unwrap());
                let start = model::data(table) * model::PAGE_BYTES + usize::from(word & 0x1fff);
                assert_eq!(
                    &output.candidate_bytes()[start..start + record.len()],
                    record
                );
            }
        }
    }
}

#[test]
fn empty_cascade_is_exact_noop_but_old_restrict_service_still_refuses_action() {
    for code in [5, 6] {
        let value = cascade(code);
        let empty: [RowDelete; 0] = [];
        let groups = [TableRowDeletes::new("Parents", &empty)];
        let output = model::capture(&value.bytes)
            .preview_ace16_cascading_deletes(&groups, &mut model::budget(), &|| false)
            .unwrap();
        assert_eq!(output.deleted_rows(), 0);
        assert_eq!(output.candidate_bytes(), value.bytes);
        assert!(matches!(
            model::capture(&value.bytes).preview_ace16_batch_deletes(
                &groups,
                &mut model::budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
    }
}
