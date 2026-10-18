//! TableCraft identities and backend-scoped record locators.
//!
//! Semantic identifiers are caller-issued opaque tokens, not native Access IDs
//! or automatically generated UUIDs. Issuers own uniqueness and persistence.
//!
//! Different identity domains cannot be passed interchangeably:
//! ```compile_fail
//! use tablecraft_core::ids::{ObjectId, TableId};
//! let table: TableId = ObjectId::from_u128(1);
//! ```
//!
//! Operation IDs and stored GUIDs do not stand in for semantic object IDs:
//! ```compile_fail
//! use tablecraft_core::ids::{ObjectId, OperationId};
//! let object: ObjectId = OperationId::from_u128(1);
//! ```
//! ```compile_fail
//! use tablecraft_core::{ids::ObjectId, values::Guid};
//! let object: ObjectId = Guid::from_bytes([0; 16]);
//! ```
//!
//! A cursor bookmark is not a record mutation key:
//! ```compile_fail
//! use tablecraft_core::ids::{CursorBookmark, CursorId, RecordId, SourceGeneration};
//! let bookmark = CursorBookmark::new(CursorId::from_u128(1), SourceGeneration::new(0), vec![1]);
//! let record: RecordId = bookmark;
//! ```

pub mod cursor;
pub mod identity;
pub mod record;

pub use cursor::CursorBookmark;
pub use identity::{CursorId, ObjectId, OperationId, SourceGeneration, TableId};
pub use record::RecordId;
