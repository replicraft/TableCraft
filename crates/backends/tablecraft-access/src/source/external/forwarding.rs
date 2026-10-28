use super::{ExternalNativeSource, LocalSource};
use crate::source::forwarding::forward_table_capabilities;
use tablecraft_query::{
    QueryError,
    source::{ExternalSourceRequest, ResolvedExternalSource, Source},
};
use tablecraft_sql::query::Query;

impl Source for LocalSource<'_> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.handle.check()?;
        self.handle.source().query(name)
    }

    forward_table_capabilities!(|this, table| this.route(table)?);
}

impl Source for ExternalNativeSource<'_> {
    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.resolve(request)
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.root.handle().check()?;
        self.root.source().query(name)
    }

    forward_table_capabilities!(|this, table| {
        this.root.handle().check()?;
        (this.root.source(), table)
    });
}
