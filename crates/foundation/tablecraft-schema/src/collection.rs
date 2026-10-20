use crate::{Column, SchemaError, SchemaName};

/// One scalar member of a named collection's child records. Members sharing a
/// collection name refer to the same child rows, not independent value lists.
/// This declaration contains no physical keys, record handles or payloads.
#[derive(Clone, PartialEq, Eq)]
pub struct CollectionMember {
    collection: SchemaName,
    column: Column,
}
impl CollectionMember {
    /// Validate the collection identity and retain an already validated scalar
    /// member declaration. Consumer width/name/case policies remain separate.
    pub fn new(collection: String, column: Column) -> Result<Self, SchemaError> {
        Ok(Self {
            collection: SchemaName::new(collection)?,
            column,
        })
    }
    /// Parent collection field's original spelling.
    pub fn collection(&self) -> &SchemaName {
        &self.collection
    }
    /// Named scalar child field and its exact carrier/nullability constraints.
    pub fn column(&self) -> &Column {
        &self.column
    }
}
