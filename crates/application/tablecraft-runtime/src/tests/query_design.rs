//! Query design is source-bound metadata and syntax, never an implicit table scan.
use super::{request, stored::opened};
use crate::{Command, Error, QueryDesignTarget, Response};
use std::sync::Arc;
use tablecraft_builders::query::{
    DesignColumn, DesignField, DesignJoin, DesignJoinKind, DesignSort, DesignSource,
    QueryDesignDraft, QueryDesignError,
};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn draft() -> QueryDesignDraft {
    let mut score = DesignColumn::field(1, "Score".into());
    score.show = false;
    score.sort = Some(DesignSort::Descending);
    score.criteria.push(">10 And <30".into());
    QueryDesignDraft {
        sources: vec![DesignSource {
            id: 1,
            table: "Items".into(),
            alias: None,
        }],
        joins: Vec::new(),
        columns: vec![DesignColumn::field(1, "Id".into()), score],
        ..QueryDesignDraft::default()
    }
}

#[test]
fn design_metadata_build_and_import_do_not_read_user_records() {
    let mut bytes = mutation_fixture::database();
    const PAGE_BYTES: usize = 4096;
    const USER_DATA_PAGE: usize = 12;
    bytes[USER_DATA_PAGE * PAGE_BYTES] = 0xff;
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };
    let Response::QueryDesignTable { metadata, .. } = request(
        &worker,
        Command::DescribeQueryDesign {
            target: target.clone(),
            table: "Items".into(),
        },
    )
    .unwrap() else {
        panic!("inert native field descriptions");
    };
    assert_eq!(
        metadata
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target: target.clone(),
            draft: Arc::new(draft()),
        },
    )
    .unwrap() else {
        panic!("owned syntax built without records");
    };
    assert!(built.sql.contains("WHERE"));
    assert!(built.sql.contains("ORDER BY [Items].[Score] DESC"));
    let Response::QueryDesignBuilt {
        built: imported, ..
    } = request(
        &worker,
        Command::ImportQueryDesign {
            target: target.clone(),
            sql: built.sql.clone(),
        },
    )
    .unwrap()
    else {
        panic!("lossless supported SQL import");
    };
    assert_eq!(built.sql, imported.sql);
    assert!(
        request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: built.sql
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn unsupported_sql_is_rejected_before_missing_native_sources_and_stale_revision_refuses() {
    let bytes = mutation_fixture::with_rows(&[]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };
    assert!(matches!(
        request(
            &worker,
            Command::ImportQueryDesign {
                target: target.clone(),
                sql: "SELECT Id FROM Missing UNION SELECT Id FROM Missing".into()
            }
        ),
        Err(Error::QueryDesign(QueryDesignError::Unsupported))
    ));
    let Response::QueryDesignBuilt { .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target: target.clone(),
            draft: Arc::new(draft()),
        },
    )
    .unwrap() else {
        panic!("empty native table still has design metadata");
    };
    request(&worker, super::open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::BuildQueryDesign {
                target,
                draft: Arc::new(draft())
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn typed_self_join_graphs_build_then_execute_with_native_outer_join_membership() {
    let bytes =
        mutation_fixture::with_rows_and_sort_order(&[(1, 1), (2, 1), (3, 99)], [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };
    let mut left = DesignColumn::field(1, "Id".into());
    left.alias = Some("LeftId".into());
    let mut right = DesignColumn::field(2, "Id".into());
    right.alias = Some("RightId".into());
    let mut draft = QueryDesignDraft {
        sources: vec![
            DesignSource {
                id: 1,
                table: "Items".into(),
                alias: Some("a".into()),
            },
            DesignSource {
                id: 2,
                table: "Items".into(),
                alias: Some("b".into()),
            },
        ],
        joins: vec![DesignJoin {
            left: DesignField {
                source_id: 1,
                column: "Id".into(),
            },
            right: DesignField {
                source_id: 2,
                column: "Score".into(),
            },
            kind: DesignJoinKind::Inner,
        }],
        columns: vec![left, right],
        ..QueryDesignDraft::default()
    };
    for (kind, expected) in [
        (DesignJoinKind::Inner, vec![["1", "1"], ["1", "2"]]),
        (
            DesignJoinKind::Left,
            vec![["1", "1"], ["1", "2"], ["2", "NULL"], ["3", "NULL"]],
        ),
        (
            DesignJoinKind::Right,
            vec![["1", "1"], ["1", "2"], ["NULL", "3"]],
        ),
    ] {
        draft.joins[0].kind = kind;
        let Response::QueryDesignBuilt {
            built, metadata, ..
        } = request(
            &worker,
            Command::BuildQueryDesign {
                target: target.clone(),
                draft: Arc::new(draft.clone()),
            },
        )
        .unwrap()
        else {
            panic!("typed self-join graph builds owned read-only SQL");
        };
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].table, "Items");
        assert_eq!(built.draft, draft);

        // Build publishes syntax only; execution is a separate explicit service.
        let Response::Data {
            source_revision,
            columns,
            rows,
            ..
        } = request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: built.sql,
            },
        )
        .unwrap()
        else {
            panic!("existing native query service executes the built graph");
        };
        assert_eq!(source_revision, None);
        assert_eq!(
            columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["LeftId", "RightId"]
        );
        let observed: Vec<_> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(observed, expected, "{kind:?}");
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

mod totals;
