//! SQL flags select only the closed ordinary or unique LONG index operation.
use crate::{Error, Response, document::Document};
use tablecraft_sql::query::definition::CreateIndex;

pub(in crate::editing) fn review(
    document: &mut Document,
    index: &CreateIndex,
    cancelled: &dyn Fn() -> bool,
) -> Result<Response, Error> {
    let [field] = index.fields.as_slice() else {
        return Err(Error::SchemaSqlUnsupported);
    };
    if index.policy.is_some() {
        return Err(Error::SchemaSqlUnsupported);
    }
    super::super::index_creation::review(
        document,
        index.table.as_str(),
        index.name.as_str(),
        field.name.as_str(),
        field.descending,
        index.unique,
        cancelled,
    )
}
