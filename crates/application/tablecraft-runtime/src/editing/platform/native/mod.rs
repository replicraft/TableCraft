//! Supported native filesystem editing services.
pub(in crate::editing) mod deleting;
pub(in crate::editing) mod draft;
pub(in crate::editing) mod ids;
pub(in crate::editing) mod input;
pub(in crate::editing) mod inserting;
pub(in crate::editing) mod query_action;
pub(in crate::editing) mod query_delete;
pub(in crate::editing) mod query_insert;
pub(in crate::editing) mod query_update;
pub(in crate::editing) mod replacing;
pub(in crate::editing) mod save;
pub(crate) mod state;
#[cfg(feature = "ace16-create")]
pub(crate) mod table_creation;

pub(in crate::editing) mod table_rename;
