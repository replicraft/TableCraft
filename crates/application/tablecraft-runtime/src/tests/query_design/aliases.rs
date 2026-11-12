//! Field-cell syntax lowers through its owner before actual runtime Build and Run.
use super::{Command, Error, QueryDesignTarget, Response, request};
use crate::tests::stored::opened;
use std::{fs, sync::Arc};
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind, DesignSort,
    DesignSource, QueryDesignDraft, edit_field_operand, field_cell_text, parse_field_cell,
};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn joined() -> QueryDesignDraft {
    let mut order = DesignColumn::field(1, "Id".into());
    order.show = false;
    order.sort = Some(DesignSort::Ascending);
    QueryDesignDraft {
        sources: vec![
            DesignSource {
                id: 1,
                table: "Items".into(),
                alias: Some("Parent".into()),
            },
            DesignSource {
                id: 2,
                table: "items".into(),
                alias: Some("Child".into()),
            },
        ],
        joins: vec![DesignJoin {
            left: DesignField {
                source_id: 1,
                column: "Id".into(),
            },
            right: DesignField {
                source_id: 2,
                column: "Id".into(),
            },
            kind: DesignJoinKind::Inner,
        }],
        columns: vec![DesignColumn::field(2, "Score".into()), order],
        ..QueryDesignDraft::default()
    }
}

#[test]
fn alias_only_edit_on_second_case_spelled_source_keeps_its_identity_and_result_heading() {
    let before = mutation_fixture::database();
    let (_directory, path, worker, revision) = opened(&before);
    let Response::QueryDesignBuilt {
        built, metadata, ..
    } = request(
        &worker,
        Command::BuildQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft: Arc::new(joined()),
        },
    )
    .unwrap()
    else {
        panic!("real source metadata and typed draft");
    };
    assert_eq!(metadata.len(), 1);
    let mut draft = built.draft;
    let (alias, operand) = parse_field_cell("[Score: view]: [Score]").unwrap();
    draft.columns[0].expression =
        edit_field_operand(&draft.sources, &metadata, Some(2), operand.into()).unwrap();
    draft.columns[0].alias = alias;
    assert_eq!(
        draft.columns[0].expression,
        DesignExpression::Field(DesignField {
            source_id: 2,
            column: "Score".into()
        })
    );
    assert_eq!(
        field_cell_text(&draft.columns[0]).unwrap(),
        "[Score: view]: Score"
    );
    let Response::QueryDesignBuilt { built, .. } = request(
        &worker,
        Command::BuildQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft: Arc::new(draft),
        },
    )
    .unwrap() else {
        panic!("alias data goes through ordinary Build");
    };
    let Response::Data {
        columns,
        rows,
        source_revision,
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
        panic!("explicit Run of actual built alias");
    };
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].name, "Score: view");
    assert_eq!(
        rows.iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["10", "20", "30"]
    );
    assert!(source_revision.is_none());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn incomplete_alias_operand_cannot_build_or_replace_its_owned_draft() {
    let before = mutation_fixture::database();
    let (_directory, path, worker, revision) = opened(&before);
    let Response::QueryDesignBuilt {
        built, metadata, ..
    } = request(
        &worker,
        Command::BuildQueryDesign {
            target: QueryDesignTarget {
                document_revision: revision,
            },
            draft: Arc::new(joined()),
        },
    )
    .unwrap()
    else {
        panic!("actual initial owner");
    };
    let mut draft = built.draft;
    let (alias, operand) = parse_field_cell("Chosen: IIf(").unwrap();
    draft.columns[0].alias = alias;
    draft.columns[0].expression =
        edit_field_operand(&draft.sources, &metadata, Some(2), operand.into()).unwrap();
    let retained = Arc::new(draft);
    // NO_CLONE: The test retains the exact immutable incomplete draft while a
    // real worker request independently owns its shared submitted witness.
    let submitted = Arc::clone(&retained);
    assert!(matches!(
        request(
            &worker,
            Command::BuildQueryDesign {
                target: QueryDesignTarget {
                    document_revision: revision
                },
                draft: submitted,
            }
        ),
        Err(Error::QueryDesign(_))
    ));
    assert_eq!(retained.columns[0].alias.as_deref(), Some("Chosen"));
    assert_eq!(
        retained.columns[0].expression,
        DesignExpression::Expression("IIf(".into())
    );
    assert_eq!(
        field_cell_text(&retained.columns[0]).unwrap(),
        "Chosen: IIf("
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!crate::journal_path(&path).exists());
}
