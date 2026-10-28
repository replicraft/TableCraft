use super::RecomputedNativeSource;
use crate::source::forwarding::forward_table_capabilities;
use tablecraft_ace::NativeReadSource;
use tablecraft_query::{QueryError, source::Source};
use tablecraft_sql::query::Query;

impl<R: NativeReadSource> Source for RecomputedNativeSource<'_, '_, R> {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Self::tick(self.source)?;
        for view in &self.views {
            Self::tick(self.source)?;
            if view.name.eq_ignore_ascii_case(name) {
                return Ok(Some(&view.query));
            }
        }
        self.source.query(name)
    }

    forward_table_capabilities!(|this, table| (this.source, table));
}
