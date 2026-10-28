use super::LinkedNativeSource;
use crate::source::forwarding::forward_table_capabilities;
use tablecraft_query::{QueryError, source::Source};
use tablecraft_sql::query::Query;

impl Source for LinkedNativeSource<'_> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.root.source().query(name)
    }

    forward_table_capabilities!(|this, table| this.route(table)?);
}
