//! Positive syntax routing never retries a rejected native target shape.
use super::*;
use tablecraft_sql::{ParseLimits, parse_query};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

#[test]
fn explicit_distinctrow_targets_resolve_actual_tables_and_self_join_occurrences() {
    for (sql, table, qualifier) in [
        (
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
            "Items",
            "t",
        ),
        (
            "DELETE DISTINCTROW r.* FROM Items AS l LEFT JOIN Items AS r ON l.Id=r.Id",
            "Items",
            "r",
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let selected =
            local_joined_delete_target(query.query(), true, &mut budget(), &|| false).unwrap();
        assert_eq!(selected.table, table);
        assert_eq!(selected.qualifier, qualifier);
    }
}

#[test]
fn unsupported_or_ambiguous_syntax_cannot_select_an_editor() {
    for (sql, expected) in [
        (
            "DELETE t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
            QueryError::Unsupported,
        ),
        (
            "DELETE DISTINCTROW t.* FROM Items AS t,Changes AS c WHERE t.Id=c.Id",
            QueryError::Unsupported,
        ),
        (
            "DELETE DISTINCTROW t.*,c.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
            QueryError::Unsupported,
        ),
        (
            "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS t ON t.Id=t.Id",
            QueryError::AmbiguousName,
        ),
        (
            "DELETE DISTINCTROW absent.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
            QueryError::UnknownName,
        ),
        (
            "DELETE DISTINCTROW t.* FROM [unopened.accdb].Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
            QueryError::Unsupported,
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            matches!(
                local_joined_delete_target(query.query(), true, &mut budget(), &|| false),
                Err(actual) if actual == expected
            ),
            "{sql}"
        );
    }
    let query = parse_query(
        "DELETE DISTINCTROW t.* FROM Items AS t INNER JOIN Changes AS c ON t.Id=c.Id",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        local_joined_delete_target(query.query(), true, &mut budget(), &|| true),
        Err(QueryError::Cancelled)
    ));
    assert!(matches!(
        local_joined_delete_target(
            query.query(),
            true,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false,
        ),
        Err(QueryError::Limit(_))
    ));
}

#[cfg(feature = "ace16-create")]
mod native {
    use super::*;
    use crate::NativeSource;
    use std::{cell::RefCell, io::Cursor};
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
        deletes, joined_update,
    };

    #[test]
    fn joined_native_selection_releases_shared_pool_and_preserves_complete_originals() {
        for (sql, removed) in [
            (
                "DELETE DISTINCTROW t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=1",
                &[0][..],
            ),
            (
                "DELETE DISTINCTROW t.* FROM Changes AS c LEFT JOIN Table1 AS t ON t.ID=c.ID AND t.ID=1 WHERE c.ID=2",
                &[][..],
            ),
        ] {
            let raw = joined_update::source();
            let expected = deletes::after(&raw.bytes, raw.target_definition, removed);
            let pool = RefCell::new(budget());
            let comparison = RefCell::new(budget());
            let mut reader = Cursor::new(raw.bytes.as_slice());
            let scope = CatalogScope::new(25001);
            let image = NativeJournalEditor::capture_image(
                &mut reader,
                scope,
                "Table1",
                &mut pool.borrow_mut(),
                &|| false,
            )
            .unwrap();
            let editor =
                NativeJournalEditor::new(image, "Table1", &mut pool.borrow_mut(), &|| false)
                    .unwrap();
            let source =
                NativeSource::new_with_shared_budget(&mut reader, scope, &pool, &|| false).unwrap();
            let (source, resolver) = source
                .with_deferred_database_collation_resolver(&comparison)
                .unwrap();
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let prepared = prepare_native_joined_delete(
                editor,
                query.query(),
                &source,
                &mut budget(),
                NativeMutationOptions {
                    number_format: None,
                    date_format: None,
                    like_mode: None,
                    parameters: &[],
                    collation: resolver.default_collation(),
                    resolver: &resolver,
                },
                &|| false,
            )
            .unwrap();
            assert_eq!(prepared.matched_rows(), removed.len());
            let plan = prepared.finish(&mut pool.borrow_mut(), &|| false).unwrap();
            assert_eq!(plan.matched_rows(), removed.len());
            assert_eq!(plan.backend().before_bytes(), raw.bytes);
            assert_eq!(plan.backend().after_bytes().len(), expected.len());
            assert_eq!(
                plan.backend()
                    .after_bytes()
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b),
                None,
                "first differing complete AFTER byte for {sql}"
            );
        }
    }
}
