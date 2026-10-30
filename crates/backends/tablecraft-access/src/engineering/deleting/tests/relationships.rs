//! Reuse the actual authored reciprocal relationship source owner.
use super::{budget, capture};
use crate::engineering::{DeleteMode, editor::tests::fixture, sql_delete};
use std::io::Cursor;
use tablecraft_ace::{
    ConstraintViolation, EditError, InspectError, RelationshipCheckStatus, check_table_constraints,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::DeleteError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access as support;

#[allow(dead_code)]
#[path = "../../../../../../engines/tablecraft-ace/tests/ace16_cascading_delete_contract/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../../../../../../engines/tablecraft-ace/tests/ace16_restrict_delete_contract/model.rs"]
mod restrict_model;

fn expected(source: &model::Model, deleted: [&[usize]; 3]) -> Vec<u8> {
    let mut after = source.bytes.clone();
    for (target, deleted_rows) in deleted.iter().enumerate() {
        if deleted_rows.is_empty() {
            continue;
        }

        let definition = model::definition(target) * 4096;
        after[definition + 16..definition + 20].copy_from_slice(
            &((source.rows[target].len() - deleted_rows.len()) as u32).to_le_bytes(),
        );
        for &row in *deleted_rows {
            let slot = model::data(target) * 4096 + 14 + row * 2;
            let original = u16::from_le_bytes(source.bytes[slot..slot + 2].try_into().unwrap());
            after[slot..slot + 2].copy_from_slice(&(original | 0xc000).to_le_bytes());
        }

        // Literal uncompressed source entries retain inactive trailing bytes.
        // Decode only entry boundaries/ROWs; never read a producer candidate.
        let page = model::root(target) * 4096;
        assert_eq!(&source.bytes[page + 24..page + 26], &[0, 0]);
        let length = 3616
            - usize::from(u16::from_le_bytes(
                source.bytes[page + 2..page + 4].try_into().unwrap(),
            ));
        let mut start = 0;
        let mut retained = Vec::new();
        for end in 1..=length {
            if source.bytes[page + 27 + end / 8] & (1 << (end % 8)) == 0 {
                continue;
            }
            let entry = &source.bytes[page + 480 + start..page + 480 + end];
            let row = usize::from(*entry.last().unwrap());
            if !deleted_rows.contains(&row) {
                retained.push(entry);
            }
            start = end;
        }
        assert_eq!(start, length);
        after[page + 27..page + 480].fill(0);
        let mut end = 0;
        for entry in retained {
            after[page + 480 + end..page + 480 + end + entry.len()].copy_from_slice(entry);
            end += entry.len();
            after[page + 27 + end / 8] |= 1 << (end % 8);
        }
        after[page + 2..page + 4].copy_from_slice(&((3616 - end) as u16).to_le_bytes());
    }
    after
}

fn qualify(bytes: &[u8]) {
    for target in 0..3 {
        let report = check_table_constraints(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            model::table(target),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(
            report
                .relationships()
                .iter()
                .all(|relationship| { relationship.status() == RelationshipCheckStatus::Checked })
        );
    }
}

#[test]
fn qualify_sql_delete_relationship_sources_and_expected_before_preview() {
    for code in [5, 6] {
        let source = model::database(code, true);
        model::before(&source);
        qualify(&source.bytes);
        qualify(&expected(&source, [&[0, 1], &[0], &[]]));
        qualify(&expected(&source, [&[], &[1], &[0]]));
        let restrict = restrict_model::database(code, false, true, false);
        qualify(&restrict.bytes);
    }
}

#[test]
fn sql_delete_restrict_refuses_and_explicit_cascade_matches_literal_closure() {
    for code in [5, 6] {
        let restrict = restrict_model::database(code, false, true, false);
        qualify(&restrict.bytes);
        let query = parse_query("DELETE FROM Parents WHERE Id=1", ParseLimits::default()).unwrap();
        assert!(matches!(
            sql_delete::preview_delete(
                capture(&restrict.bytes),
                query.query(),
                DeleteMode::Restrict,
                &mut budget(),
                None,
                &|| false
            ),
            Err(DeleteError::Backend(crate::NativeEditError::Edit(
                EditError::Inspect(InspectError::Constraint(ConstraintViolation::MissingParent))
            )))
        ));

        let source = model::database(code, true);
        qualify(&source.bytes);
        for (id, selected) in [
            (1, [&[0, 1][..], &[0][..], &[][..]]),
            (2, [&[][..], &[1][..], &[0][..]]),
        ] {
            let after = expected(&source, selected);
            qualify(&after);
            let query = parse_query(
                &format!("DELETE FROM Parents WHERE Id={id}"),
                ParseLimits::default(),
            )
            .unwrap();
            let plan = sql_delete::preview_delete(
                capture(&source.bytes),
                query.query(),
                DeleteMode::Cascading,
                &mut budget(),
                None,
                &|| false,
            )
            .unwrap();
            assert_eq!(plan.matched_rows(), 1);
            let copy = plan.into_backend();
            assert_eq!(
                copy.affected_rows(),
                selected.iter().map(|rows| rows.len()).sum::<usize>()
            );
            assert_eq!(copy.candidate_bytes(), after);
        }
    }
}
