//! Mechanical table-capability forwarding; callers supply their explicit route.
//!
//! Named queries, external registries, namespace ownership and handle validation
//! remain in each adapter. This macro adds no routing or fallback policy.
macro_rules! forward_table_capabilities {
    (|$this:ident, $table:ident| $route:expr) => {
        fn text_equality_hints(&self, table: &str) -> Result<bool, tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.text_equality_hints(table)
        }

        fn columns(
            &self,
            table: &str,
        ) -> Result<&[tablecraft_schema::Column], tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.columns(table)
        }

        fn text_collations(
            &self,
            table: &str,
        ) -> Result<
            &[Option<tablecraft_query::source::TextCollationId>],
            tablecraft_query::QueryError,
        > {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.text_collations(table)
        }

        fn unavailable_columns(
            &self,
            table: &str,
        ) -> Result<&[tablecraft_schema::SchemaName], tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.unavailable_columns(table)
        }

        fn multivalue_columns(
            &self,
            table: &str,
        ) -> Result<&[tablecraft_schema::Column], tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.multivalue_columns(table)
        }

        fn multivalue_text_collations(
            &self,
            table: &str,
        ) -> Result<
            &[Option<tablecraft_query::source::TextCollationId>],
            tablecraft_query::QueryError,
        > {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.multivalue_text_collations(table)
        }

        fn collection_members(
            &self,
            table: &str,
        ) -> Result<&[tablecraft_schema::CollectionMember], tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.collection_members(table)
        }

        fn collection_member_text_collations(
            &self,
            table: &str,
        ) -> Result<
            &[Option<tablecraft_query::source::TextCollationId>],
            tablecraft_query::QueryError,
        > {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.collection_member_text_collations(table)
        }

        fn scan_multivalues(
            &self,
            table: &str,
            indices: &[usize],
            collections: &[usize],
            visitor: &mut tablecraft_query::source::MultivalueVisitor<'_>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan_multivalues(table, indices, collections, visitor)
        }

        fn scan_collection_members(
            &self,
            table: &str,
            indices: &[usize],
            values: &[usize],
            members: &[usize],
            visitor: &mut tablecraft_query::source::MultivalueVisitor<'_>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan_collection_members(table, indices, values, members, visitor)
        }

        fn scan(
            &self,
            table: &str,
            visitor: &mut dyn FnMut(
                &[tablecraft_core::values::Value],
            ) -> Result<(), tablecraft_query::QueryError>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan(table, visitor)
        }

        fn scan_projected(
            &self,
            table: &str,
            indices: &[usize],
            constraint: Option<tablecraft_query::source::ScanConstraint>,
            visitor: &mut dyn FnMut(
                tablecraft_db::ProjectedRow<'_>,
            ) -> Result<(), tablecraft_query::QueryError>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan_projected(table, indices, constraint, visitor)
        }

        fn scan_projected_progressive(
            &self,
            table: &str,
            indices: &[usize],
            visitor: &mut dyn FnMut(
                tablecraft_db::ProjectedRow<'_>,
            ) -> Result<(), tablecraft_query::QueryError>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan_projected_progressive(table, indices, visitor)
        }

        fn scan_constrained(
            &self,
            table: &str,
            constraint: tablecraft_query::source::ScanConstraint,
            visitor: &mut dyn FnMut(
                &[tablecraft_core::values::Value],
            ) -> Result<(), tablecraft_query::QueryError>,
        ) -> Result<(), tablecraft_query::QueryError> {
            let $this = self;
            let $table = table;
            let (source, table) = $route;

            source.scan_constrained(table, constraint, visitor)
        }
    };
}

pub(super) use forward_table_capabilities;
