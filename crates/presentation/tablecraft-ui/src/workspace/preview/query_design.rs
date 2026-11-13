//! Authored design inventories are presentation fixtures, never executed queries.
use super::{Preview, fixtures};
use crate::{
    ObjectKind, ObjectTarget, QueryDesignCompletion, QueryDesignRequest, SavedQuerySqlEligibility,
};
use std::sync::Arc;
use tablecraft_builders::query::{
    BuiltQueryDesign, DesignColumn, DesignField, DesignFieldMetadata, DesignJoin, DesignJoinKind,
    DesignSort, DesignSource, DesignTableMetadata, DesignTotal, QueryDesignDraft, build,
};

impl Preview {
    pub(super) fn prepare_query_design_totals(&mut self) {
        self.prepare_query_design(false, false);
        let mut key = DesignColumn::field(1, "CustomerName".into());
        key.sort = Some(DesignSort::Ascending);
        let mut counted = DesignColumn::field(1, "CustomerID".into());
        counted.total = DesignTotal::Count;
        counted.alias = Some("CustomerCount".into());
        let mut filtered = DesignColumn::field(1, "CustomerID".into());
        filtered.total = DesignTotal::Where;
        filtered.show = false;
        filtered.criteria.push(">0".into());
        let draft = QueryDesignDraft {
            totals: true,
            parameters: Vec::new(),
            sources: vec![DesignSource {
                id: 1,
                table: "Customers".into(),
                alias: None,
            }],
            joins: Vec::new(),
            columns: vec![key, counted, filtered],
        };
        let metadata = vec![table_metadata("Customers")];
        let built = build(&draft, &metadata).expect("authored grouped QBE inventory");
        // NO_CLONE: The editable SQL and the separately owned import reply retain
        // independent spellings until the normal exact pending comparison completes.
        self.state.sql = built.sql.clone();
        let request = self
            .state
            .query_design_request()
            .expect("synthetic scratch tab");
        let id = self.request();
        let sql = self
            .state
            .capture_query_draft()
            .expect("bounded authored SQL");
        assert!(self.state.begin_query_design_import(request, sql, id));
        assert!(
            self.state
                .set_query_design_built(id, built, metadata)
                .is_some()
        );
    }

    pub(super) fn prepare_wide_query_design(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        let mut columns = Vec::new();
        for position in 0..32 {
            let mut column = if position % 2 == 0 {
                DesignColumn::field(1, "CustomerName".into())
            } else {
                DesignColumn::field(2, "OrderTotal".into())
            };
            column.alias = Some(format!(
                "Synthetic_projection_with_a_long_caption_{position:02}"
            ));
            column.show = position != 1;
            if position == 0 {
                column.sort = Some(DesignSort::Ascending);
                column.criteria = vec!["Like \"A*\"".into(); 8];
            }
            if position == 1 {
                column.criteria = vec![">= 0".into(); 8];
            }
            columns.push(column);
        }
        let draft = QueryDesignDraft {
            totals: false,
            parameters: Vec::new(),
            sources: vec![
                DesignSource {
                    id: 1,
                    table: "Customers".into(),
                    alias: None,
                },
                DesignSource {
                    id: 2,
                    table: "Orders".into(),
                    alias: None,
                },
            ],
            joins: vec![DesignJoin {
                left: DesignField {
                    source_id: 1,
                    column: "CustomerID".into(),
                },
                right: DesignField {
                    source_id: 2,
                    column: "CustomerID".into(),
                },
                kind: DesignJoinKind::Left,
            }],
            columns,
        };
        let mut metadata = vec![table_metadata("Customers"), table_metadata("Orders")];
        for table in &mut metadata {
            table
                .fields
                .extend((0..20).map(|index| DesignFieldMetadata {
                    name: format!("Synthetic_scrollable_field_{index:02}"),
                    scalar_type: None,
                }));
        }
        let built = build(&draft, &metadata).expect("bounded authored synthetic QBE");
        // NO_CLONE: Current SQL editor and BuiltQueryDesign reply retain independent SQL
        // spellings during import; later reply consumes built SQL after comparing pending
        // editor snapshot.
        self.state.sql = built.sql.clone();
        let request = self
            .state
            .query_design_request()
            .expect("synthetic scratch tab");
        let id = self.request();
        // NO_CLONE: PendingKind::Import owns exact editor SQL snapshot for stale-input
        // comparison while visible editor remains mutable; no borrowed editor lifetime across
        // pending state.
        assert!(
            self.state
                .begin_query_design_import(request, self.state.sql.clone(), id)
        );
        assert!(
            self.state
                .set_query_design_built(id, built, metadata)
                .is_some()
        );
    }

    pub(super) fn prepare_empty_query_design(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        self.state.sql.clear();
        let request = self
            .state
            .query_design_request()
            .expect("synthetic scratch tab");
        let id = self.request();
        assert!(
            self.state
                .begin_query_design_import(request, String::new(), id)
        );
        // This authored empty editor model has no executable query or native
        // metadata. Use the same source-bound fixture publication as the other
        // Design scenes, without fabricating a table or running a builder.
        assert!(
            self.state
                .set_query_design_built(
                    id,
                    BuiltQueryDesign {
                        draft: QueryDesignDraft::default(),
                        sql: String::new()
                    },
                    Vec::new(),
                )
                .is_some()
        );
    }

    pub(super) fn prepare_query_design(&mut self, tables: bool, error: bool) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        self.state.sql = "SELECT [CustomerID], [CustomerName] FROM [Customers]".into();
        let request = self
            .state
            .query_design_request()
            .expect("synthetic scratch tab");
        let id = self.request();
        // NO_CLONE: Pending import must retain exact editor snapshot independently of editor
        // and built result; this instance exercises normal authored QBE import.
        assert!(
            self.state
                .begin_query_design_import(request, self.state.sql.clone(), id)
        );
        let mut name = DesignColumn::field(1, "CustomerName".into());
        name.criteria = vec!["Like \"A*\"".into()];
        let draft = QueryDesignDraft {
            totals: false,
            parameters: Vec::new(),
            sources: vec![DesignSource {
                id: 1,
                table: "Customers".into(),
                alias: None,
            }],
            joins: Vec::new(),
            columns: vec![DesignColumn::field(1, "CustomerID".into()), name],
        };
        // NO_CLONE: BuiltQueryDesign reply owns proposed SQL while State retains
        // pending/current SQL until admission; final reply moves its SQL into editor.
        assert!(
            self.state
                .set_query_design_built(
                    id,
                    BuiltQueryDesign {
                        draft,
                        sql: self.state.sql.clone()
                    },
                    vec![table_metadata("Customers")],
                )
                .is_some()
        );
        if tables {
            self.state.show_query_design_tables();
        }
        if error {
            let request = self
                .state
                .query_design_request()
                .expect("same synthetic tab");
            // NO_CLONE: Simulated failing build owns draft snapshot while visible QBE draft
            // stays in State for rejected-build preservation; moving would discard current
            // design.
            let draft = self
                .state
                .query_design_draft()
                .expect("synthetic draft")
                .clone();
            let id = self.request();
            assert!(self.state.begin_query_design_build(
                request,
                Arc::new(draft),
                QueryDesignCompletion::ShowSql,
                id
            ));
            self.state
                .fail_query_design(id, self.state.localizer.text("qbe-mock-unavailable"));
        }
    }

    pub(super) fn mock_describe_query_design(
        &mut self,
        request: QueryDesignRequest,
        table: String,
    ) {
        let id = self.request();
        // NO_CLONE: Pending table description owns table name while fixture metadata generation
        // later reads original after admission; result owns its independent table spelling.
        if self
            .state
            .begin_query_design_description(request, table.clone(), id)
        {
            self.state
                .set_query_design_table(id, table_metadata(&table));
        }
    }
}

fn table_metadata(table: &str) -> DesignTableMetadata {
    DesignTableMetadata {
        table: table.into(),
        fields: fixtures::page(table, 0)
            .columns
            .into_iter()
            .map(|column| DesignFieldMetadata {
                name: column.name,
                scalar_type: None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests;

impl Preview {
    pub(super) fn prepare_query_save_name(&mut self) {
        self.mock_open_object(ObjectTarget {
            document_revision: fixtures::REVISION,
            kind: ObjectKind::Sql,
        });
        assert!(self.state.replace_query_sql("SELECT 99 AS [Answer]".into()));
        assert!(self.state.begin_query_creation_name());
        assert!(self.state.set_query_creation_name("Query1".into()));
    }

    pub(super) fn prepare_saved_query_design(&mut self) {
        let target = fixtures::saved_target();
        let id = self.request();
        // NO_CLONE: The synthetic read observation and its independent reply
        // comparison retain the same bounded identity, never a native capability.
        self.state.begin_saved_query_inspection(target.clone(), id);
        assert!(self.state.set_saved_query_details(id, &target, Vec::new()));
        let id = self.request();
        let inspection = self
            .state
            .begin_saved_query_sql_inspection(target, id)
            .expect("synthetic saved owner");
        let sql = "SELECT 99 AS [Answer]";
        assert!(self.state.set_saved_query_sql_inspection(
            id,
            &inspection,
            Some(sql.into()),
            SavedQuerySqlEligibility::Unavailable
        ));
        let built = tablecraft_builders::query::import(sql, &[]).expect("constant saved design");
        let request = self
            .state
            .query_design_request()
            .expect("actual saved Design owner");
        let id = self.request();
        assert!(
            self.state
                .begin_query_design_import(request, sql.into(), id)
        );
        assert!(
            self.state
                .set_query_design_built(id, built, Vec::new())
                .is_some()
        );
    }
}
