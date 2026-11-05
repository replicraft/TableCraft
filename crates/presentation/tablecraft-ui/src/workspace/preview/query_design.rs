//! Authored design inventories are presentation fixtures, never executed queries.
use super::{Preview, fixtures};
use crate::{ObjectKind, ObjectTarget, QueryDesignCompletion, QueryDesignRequest};
use tablecraft_builders::query::{
    BuiltQueryDesign, DesignColumn, DesignFieldMetadata, DesignSource, DesignTableMetadata,
    QueryDesignDraft,
};

impl Preview {
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
        assert!(
            self.state
                .begin_query_design_import(request, self.state.sql.clone(), id)
        );
        let mut name = DesignColumn::field(1, "CustomerName".into());
        name.criteria = vec!["Like \"A*\"".into()];
        let draft = QueryDesignDraft {
            sources: vec![DesignSource {
                id: 1,
                table: "Customers".into(),
                alias: None,
            }],
            joins: Vec::new(),
            columns: vec![DesignColumn::field(1, "CustomerID".into()), name],
        };
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
            let draft = self
                .state
                .query_design_draft()
                .expect("synthetic draft")
                .clone();
            let id = self.request();
            assert!(self.state.begin_query_design_build(
                request,
                draft,
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
