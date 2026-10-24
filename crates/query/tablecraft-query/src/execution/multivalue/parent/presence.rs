//! Presence evaluates at its written scalar position under the borrowed parent frame.
use super::{ParentCollections, QueryError, Runtime};
use tablecraft_core::values::Value;
use tablecraft_schema::validate_row;

impl Runtime<'_, '_> {
    pub(in crate::execution) fn collection_presence(
        &mut self,
        collection: usize,
        negated: bool,
        collections: &ParentCollections<'_>,
    ) -> Result<Value, QueryError> {
        self.tick()?;
        let position = collections
            .requested
            .binary_search(&collection)
            .map_err(|_| QueryError::Source)?;
        let absent = collections.absent.is_some_and(|missing| missing[position]);
        let list = collections.lists.get(position).ok_or(QueryError::Source)?;
        let column = collections
            .columns
            .get(position)
            .ok_or(QueryError::Source)?;
        if !absent {
            for index in 0..list.len() {
                self.tick()?;
                let value = list.get(index).ok_or(QueryError::Source)?;
                validate_row(std::slice::from_ref(column), std::slice::from_ref(value))?;
            }
        }
        self.tick()?;
        Ok(Value::Boolean((absent || list.is_empty()) != negated))
    }
}
