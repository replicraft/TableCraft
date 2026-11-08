//! Public facts and explicit refusals without native editing authority.
pub(in crate::editing) mod deleting;
pub(in crate::editing) mod draft;
pub(in crate::editing) mod inserting;
pub(in crate::editing) mod query_delete;
pub(in crate::editing) mod query_insert;
pub(in crate::editing) mod query_update;
pub(in crate::editing) mod replacing;
pub(in crate::editing) mod save;
pub(crate) mod state;
#[cfg(feature = "ace16-create")]
pub(crate) mod table_creation;
