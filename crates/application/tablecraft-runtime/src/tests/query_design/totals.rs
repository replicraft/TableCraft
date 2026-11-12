//! Actual design services retain grouping, prefilters and declared inputs through Run.
use super::{Command, Error, QueryDesignTarget, Response, request};
use crate::ParameterInput;
use crate::tests::{open, query_parameters::inspect, stored::opened};
use std::{fs, sync::Arc};
use tablecraft_builders::query::{
    DesignColumn, DesignSource, DesignTotal, QueryDesignDraft, QueryDesignError,
};
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryError;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn draft() -> QueryDesignDraft {
    let parameters = parse_query(
        "PARAMETERS [Minimum] LONG; SELECT 1",
        ParseLimits::default(),
    )
    .unwrap()
    .into_query()
    .parameters;
    let key = DesignColumn::field(1, "Score".into());
    let mut total = DesignColumn::field(1, "Id".into());
    total.total = DesignTotal::Sum;
    total.alias = Some("Total".into());
    total.criteria.push(">= [Minimum]".into());

    let mut prefilter = DesignColumn::field(1, "Id".into());
    prefilter.total = DesignTotal::Where;
    prefilter.show = false;
    prefilter.criteria.push("> 1".into());

    QueryDesignDraft {
        totals: true,
        parameters,
        sources: vec![DesignSource {
            id: 1,
            table: "Items".into(),
            alias: None,
        }],
        joins: Vec::new(),
        columns: vec![key, total, prefilter],
    }
}

#[test]
fn actual_build_import_and_typed_run_keep_where_separate_from_having() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 1), (2, 1), (3, 2)], [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };

    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft: Arc::new(draft()),
        },
    )
    .unwrap() else {
        panic!("owned native design build");
    };
    assert!(built.draft.totals);
    assert_eq!(built.draft.parameters.len(), 1);
    assert_eq!(built.draft.columns[1].total, DesignTotal::Sum);
    assert_eq!(built.draft.columns[2].total, DesignTotal::Where);
    assert!(built.sql.contains("WHERE"));
    assert!(built.sql.contains("GROUP BY"));
    assert!(built.sql.contains("HAVING"));
    assert_eq!(fs::read(&path).unwrap(), bytes);

    let Response::QueryDesignBuilt {
        built: imported, ..
    } = request(
        &worker,
        Command::ImportQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            sql: built.sql,
        },
    )
    .unwrap()
    else {
        panic!("same grouped declaration imports");
    };
    assert!(imported.draft.totals);
    assert_eq!(imported.draft.parameters.len(), 1);
    assert_eq!(imported.draft.parameters[0].name.as_str(), "Minimum");
    assert!(
        imported
            .draft
            .columns
            .iter()
            .any(|column| column.total == DesignTotal::Where)
    );
    let Response::QueryDesignBuilt { built: rebuilt, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target,
            draft: Arc::new(imported.draft),
        },
    )
    .unwrap() else {
        panic!("actual rebuilt grouped design");
    };
    assert_eq!(rebuilt.sql, imported.sql);

    for (minimum, expected) in [("3", vec![vec!["2", "3"]]), ("4", Vec::new())] {
        let (target, specs) = inspect(&worker, revision, &rebuilt.sql);
        assert_eq!(specs.len(), 1);
        assert_eq!(
            (specs[0].name.as_str(), specs[0].kind.as_str()),
            ("Minimum", "int32")
        );
        let Response::Data {
            rows,
            source_revision,
            ..
        } = request(
            &worker,
            Command::RunQuery {
                target,
                options: Default::default(),
                parameters: vec![ParameterInput {
                    name: "Minimum".into(),
                    kind: "int32".into(),
                    text: minimum.into(),
                    is_null: false,
                }],
            },
        )
        .unwrap()
        else {
            panic!("explicit typed grouped Run");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(source_revision.is_none());
    }
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn grouped_build_is_metadata_only_and_stale_revision_precedes_bad_input() {
    let mut bytes = mutation_fixture::database();
    bytes[12 * 4096] = 0xff;
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };

    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft: Arc::new(draft()),
        },
    )
    .unwrap() else {
        panic!("grouped metadata without user DATA");
    };
    assert!(built.draft.totals);
    let Response::QueryDesignBuilt { .. } = request(
        &worker,
        Command::ImportQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            sql: built.sql,
        },
    )
    .unwrap() else {
        panic!("inert grouping import");
    };
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ImportQueryDesign {
                target,
                sql: "not SQL".into(),
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn late_group_failure_and_unsupported_import_leave_source_and_owned_design_unchanged() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 1), (2, 2), (3, 2)], [9, 4, 0, 0]);
    let (_directory, path, worker, revision) = opened(&bytes);
    let target = QueryDesignTarget {
        document_revision: revision,
    };

    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::ImportQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            sql: "SELECT Score,Sum(1/(Id-3)) AS Total FROM Items GROUP BY Score".into(),
        },
    )
    .unwrap() else {
        panic!("inert design import does not evaluate rows");
    };
    let retained = Arc::new(built.draft);
    assert!(matches!(
        request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: built.sql
            }
        ),
        Err(Error::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert!(matches!(
        request(
            &worker,
            Command::ImportQueryDesign {
                target: QueryDesignTarget {
                    document_revision: revision
                },
                sql: "SELECT Id FROM Items UNION SELECT Id FROM Items".into(),
            }
        ),
        Err(Error::QueryDesign(QueryDesignError::Unsupported))
    ));
    // NO_CLONE: The request and retained test witness share the same immutable
    // owned draft to prove failed import did not replace or mutate its data.

    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target,
            draft: Arc::clone(&retained),
        },
    )
    .unwrap() else {
        panic!("original owned design remains buildable");
    };
    assert_eq!(&built.draft, retained.as_ref());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(!crate::journal_path(&path).exists());
}
