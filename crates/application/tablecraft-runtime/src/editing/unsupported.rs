//! Public facts and explicit refusals without native editing authority.
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod add_column;
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod database_options;
pub(in crate::editing) mod deleting;
pub(in crate::editing) mod draft;
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod field_addition;
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod field_drop;
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod field_rename;
pub(in crate::editing) mod inserting;
#[cfg(feature = "ace16-create")]
pub(in crate::editing) mod number_column;
pub(in crate::editing) mod query_delete;
pub(in crate::editing) mod query_expression;
pub(in crate::editing) mod query_insert;
pub(in crate::editing) mod query_sql;
pub(in crate::editing) mod query_update;
pub(in crate::editing) mod replacing;
pub(in crate::editing) mod save;
pub(crate) mod state;
#[cfg(feature = "ace16-create")]
pub(crate) mod table_creation;

pub(in crate::editing) mod table_drop;
pub(in crate::editing) mod table_rename;
