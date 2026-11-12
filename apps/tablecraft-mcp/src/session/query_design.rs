//! Headless design uses checked builder drafts and actual native metadata.
use super::{Session, arguments as arg};
use serde_json::{Value, json};
use std::sync::Arc;
use tablecraft_builders::query::{
    DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind, DesignSort,
    DesignSource, DesignTableMetadata, DesignTotal, MAX_DESIGN_COLUMNS, MAX_DESIGN_CRITERIA_ROWS,
    MAX_DESIGN_JOINS, MAX_DESIGN_SOURCES, QueryDesignDraft, parse_parameters, render_parameters,
};
use tablecraft_runtime::{Command, QueryDesignTarget, Response};

#[derive(Default)]
pub(super) struct DesignState {
    draft: Option<(u64, u64, Arc<QueryDesignDraft>)>,
}

impl Session {
    pub(super) fn design_operation(
        &mut self,
        operation: &str,
        args: &Value,
    ) -> Result<Value, String> {
        let revision = self.current_revision(args)?;
        let target = QueryDesignTarget {
            document_revision: revision,
        };
        let command = match operation {
            "describe_query_design" => Command::DescribeQueryDesign {
                target,
                table: arg::owned(args, "table")?,
            },
            "import_query_design" => Command::ImportQueryDesign {
                target,
                sql: arg::owned(args, "sql")?,
            },
            "build_query_design" => Command::BuildQueryDesign {
                target,
                draft: Arc::new(parse_draft(args.get("draft").ok_or("draft is required")?)?),
            },
            "rebuild_query_design" => {
                let requested = arg::u64_value(args, "design_id")?;
                let Some((_, _, draft)) = self
                    .design_state
                    .draft
                    .as_ref()
                    .filter(|(id, owner, _)| *id == requested && *owner == revision)
                else {
                    return Err("Unknown, superseded or stale query-design handle".into());
                };
                // NO_CLONE: A rebuild command independently owns this immutable checked draft
                // across the worker boundary; the host retains it if fresh validation fails.
                let draft = Arc::clone(draft);
                Command::BuildQueryDesign { target, draft }
            }
            _ => return Err("Unknown query-design operation".into()),
        };
        match self.request(command)? {
            Response::QueryDesignTable {
                target,
                metadata: table,
            } => Ok(
                json!({"document_revision": target.document_revision, "metadata": metadata(&table)}),
            ),
            Response::QueryDesignBuilt {
                target,
                built,
                metadata: tables,
            } => {
                if target.document_revision != revision {
                    return Err(
                        "The query-design response belongs to another document revision".into(),
                    );
                }
                let id = self.next_handle()?;
                let draft = draft_json(&built.draft)?;
                let result = json!({
                    "document_revision": revision,
                    "design_id": id,
                    "sql": built.sql,
                    "draft": draft,
                    "metadata": tables.iter().map(metadata).collect::<Vec<_>>(),
                });
                self.design_state.draft = Some((id, revision, Arc::new(built.draft)));
                Ok(result)
            }
            _ => Err("The runtime returned another query-design response".into()),
        }
    }
}

fn metadata(table: &DesignTableMetadata) -> Value {
    json!({"table": table.table, "fields": table.fields.iter().map(|field| json!({"name": field.name, "scalar_type": field.scalar_type.map(|kind| format!("{kind:?}"))})).collect::<Vec<_>>()})
}

fn field_json(field: &DesignField) -> Value {
    json!({"source_id": field.source_id, "column": field.column})
}

fn draft_json(draft: &QueryDesignDraft) -> Result<Value, String> {
    let parameters_sql = render_parameters(&draft.parameters).map_err(|error| error.to_string())?;
    let sources = draft
        .sources
        .iter()
        .map(|source| {
            json!({
                "id": source.id,
                "table": source.table,
                "alias": source.alias,
            })
        })
        .collect::<Vec<_>>();
    let joins = draft
        .joins
        .iter()
        .map(|join| {
            json!({
                "left": field_json(&join.left),
                "right": field_json(&join.right),
                "kind": match join.kind {
                    DesignJoinKind::Inner => "inner",
                    DesignJoinKind::Left => "left",
                    DesignJoinKind::Right => "right",
                },
            })
        })
        .collect::<Vec<_>>();
    let columns = draft
        .columns
        .iter()
        .map(|column| {
            let expression = match &column.expression {
                DesignExpression::Field(field) => json!({
                    "kind": "field",
                    "source_id": field.source_id,
                    "column": field.column,
                }),
                DesignExpression::AllFields(source_id) => json!({
                    "kind": "all_fields",
                    "source_id": source_id,
                }),
                DesignExpression::Expression(text) => json!({
                    "kind": "expression",
                    "text": text,
                }),
            };
            json!({
                "expression": expression,
                "total": total_name(column.total),
                "alias": column.alias,
                "show": column.show,
                "sort": column.sort.map(|sort| match sort {
                    DesignSort::Ascending => "ascending",
                    DesignSort::Descending => "descending",
                }),
                "criteria": column.criteria,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "totals": draft.totals,
        "parameters_sql": parameters_sql,
        "sources": sources,
        "joins": joins,
        "columns": columns,
    }))
}

fn total_name(total: DesignTotal) -> &'static str {
    match total {
        DesignTotal::GroupBy => "group_by",
        DesignTotal::Sum => "sum",
        DesignTotal::Avg => "avg",
        DesignTotal::Min => "min",
        DesignTotal::Max => "max",
        DesignTotal::Count => "count",
        DesignTotal::First => "first",
        DesignTotal::Last => "last",
        DesignTotal::Expression => "expression",
        DesignTotal::Where => "where",
    }
}

fn parse_total(column: &Value) -> Result<DesignTotal, String> {
    Ok(match column.get("total").filter(|value| !value.is_null()) {
        None => DesignTotal::GroupBy,
        Some(value) => match value.as_str().ok_or("total must be a string")? {
            "group_by" => DesignTotal::GroupBy,
            "sum" => DesignTotal::Sum,
            "avg" => DesignTotal::Avg,
            "min" => DesignTotal::Min,
            "max" => DesignTotal::Max,
            "count" => DesignTotal::Count,
            "first" => DesignTotal::First,
            "last" => DesignTotal::Last,
            "expression" => DesignTotal::Expression,
            "where" => DesignTotal::Where,
            _ => return Err("Unsupported design Total role".into()),
        },
    })
}

fn array<'a>(args: &'a Value, key: &str, limit: usize) -> Result<&'a [Value], String> {
    let values = args
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be an array"))?;
    if values.len() > limit {
        return Err(format!("{key} exceeds the checked design limit"));
    }
    Ok(values)
}

fn optional_text(args: &Value, key: &str) -> Result<Option<String>, String> {
    args.get(key)
        .filter(|value| !value.is_null())
        .map(|_| arg::owned(args, key))
        .transpose()
}

fn field(args: &Value) -> Result<DesignField, String> {
    Ok(DesignField {
        source_id: arg::u64_value(args, "source_id")?,
        column: arg::owned(args, "column")?,
    })
}

fn parse_draft(args: &Value) -> Result<QueryDesignDraft, String> {
    let sources = array(args, "sources", MAX_DESIGN_SOURCES)?
        .iter()
        .map(|source| {
            Ok(DesignSource {
                id: arg::u64_value(source, "id")?,
                table: arg::owned(source, "table")?,
                alias: optional_text(source, "alias")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let joins = array(args, "joins", MAX_DESIGN_JOINS)?
        .iter()
        .map(|join| {
            let kind = match arg::text(join, "kind")? {
                "inner" => DesignJoinKind::Inner,
                "left" => DesignJoinKind::Left,
                "right" => DesignJoinKind::Right,
                _ => return Err("Unsupported join kind".into()),
            };
            Ok(DesignJoin {
                left: field(join.get("left").ok_or("left is required")?)?,
                right: field(join.get("right").ok_or("right is required")?)?,
                kind,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let columns = array(args, "columns", MAX_DESIGN_COLUMNS)?
        .iter()
        .map(|column| {
            let value = column.get("expression").ok_or("expression is required")?;
            let expression = match arg::text(value, "kind")? {
                "field" => DesignExpression::Field(field(value)?),
                "all_fields" => DesignExpression::AllFields(
                    value
                        .get("source_id")
                        .filter(|value| !value.is_null())
                        .map(|_| arg::u64_value(value, "source_id"))
                        .transpose()?,
                ),
                "expression" => DesignExpression::Expression(arg::owned(value, "text")?),
                _ => return Err("Unsupported design expression kind".into()),
            };
            let sort = match column.get("sort").filter(|value| !value.is_null()) {
                None => None,
                Some(value) => Some(match value.as_str().ok_or("sort must be a string")? {
                    "ascending" => DesignSort::Ascending,
                    "descending" => DesignSort::Descending,
                    _ => return Err("Unsupported design sort".into()),
                }),
            };
            let criteria = match arg::optional(column, "criteria") {
                None => Vec::new(),
                Some(_) => array(column, "criteria", MAX_DESIGN_CRITERIA_ROWS)?
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .map(String::from)
                            .ok_or_else(|| "criteria must contain strings".into())
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            };
            Ok(DesignColumn {
                expression,
                total: parse_total(column)?,
                alias: optional_text(column, "alias")?,
                show: arg::boolean(column, "show", true)?,
                sort,
                criteria,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let parameters = parse_parameters(
        arg::optional(args, "parameters_sql")
            .map(|_| arg::text(args, "parameters_sql"))
            .transpose()?
            .unwrap_or(""),
    )
    .map_err(|error| error.to_string())?;
    Ok(QueryDesignDraft {
        totals: arg::boolean(args, "totals", false)?,
        parameters,
        sources,
        joins,
        columns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::opened;
    use tablecraft_testkit::fixtures::access::mutation_fixture;

    #[test]
    fn real_metadata_import_owned_draft_rebuild_and_explicit_execution_are_separate() {
        let source = mutation_fixture::database();
        let (_directory, path, mut session) = opened(&source);
        let revision = session.revision.unwrap();
        let description = session
            .execute(
                "describe_query_design",
                json!({"document_revision": revision, "table": "Items"}),
            )
            .unwrap();
        assert_eq!(description["metadata"]["fields"][0]["name"], "Id");
        let imported = session.execute("import_query_design", json!({"document_revision": revision, "sql": "SELECT [Id] FROM [Items] WHERE [Score] > 10 ORDER BY [Score] DESC;"})).unwrap();
        assert!(imported["sql"].as_str().unwrap().contains("WHERE"));
        let rebuilt = session
            .execute(
                "rebuild_query_design",
                json!({"document_revision": revision, "design_id": imported["design_id"]}),
            )
            .unwrap();
        assert_eq!(rebuilt["sql"], imported["sql"]);
        assert!(
            session
                .execute(
                    "rebuild_query_design",
                    json!({"document_revision": revision, "design_id": imported["design_id"]})
                )
                .is_err()
        );
        let built = session
            .execute(
                "build_query_design",
                json!({"document_revision": revision, "draft": rebuilt["draft"]}),
            )
            .unwrap();
        assert_eq!(built["sql"], rebuilt["sql"]);
        assert!(!session.saves.is_busy());
        assert_eq!(std::fs::read(&path).unwrap(), source);
        let inspected = session
            .execute(
                "inspect_query",
                json!({"document_revision": revision, "sql": built["sql"]}),
            )
            .unwrap();
        let result = session
            .execute(
                "run_query",
                json!({"document_revision": revision, "query_id": inspected["query_id"]}),
            )
            .unwrap();
        assert!(result["rows"].is_array());
        session
            .execute("open_database", json!({"path": path}))
            .unwrap();
        assert!(session.execute("rebuild_query_design", json!({"document_revision": session.revision.unwrap(), "design_id": built["design_id"]})).is_err());
    }

    #[test]
    fn unsupported_sql_and_unknown_native_fields_never_publish_a_design_or_write() {
        let source = mutation_fixture::database();
        let (_directory, path, mut session) = opened(&source);
        let revision = session.revision.unwrap();
        assert!(
            session
                .execute(
                    "import_query_design",
                    json!({"document_revision": revision, "sql": "DELETE FROM [Items];"})
                )
                .is_err()
        );
        assert!(session.execute("build_query_design", json!({"document_revision": revision, "draft": {
            "sources": [{"id": 1, "table": "Items"}], "joins": [],
            "columns": [{"expression": {"kind": "field", "source_id": 1, "column": "Missing"}}]
        }})).is_err());
        assert!(session.design_state.draft.is_none());
        assert!(!session.saves.is_busy());
        assert_eq!(std::fs::read(path).unwrap(), source);
    }
}

#[cfg(test)]
mod totals_tests;

#[cfg(test)]
mod alias_tests;
