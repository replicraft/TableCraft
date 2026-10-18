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

use std::fmt;

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name(u128);

        impl $name {
            /// Wrap a caller-issued token without changing its identity domain.
            pub const fn from_u128(token: u128) -> Self {
                Self(token)
            }

            /// Return the token for an explicit mapping or protocol encoder.
            pub const fn as_u128(self) -> u128 {
                self.0
            }
        }
    };
}

identity!(ObjectId, "A semantic application-object identity.");
identity!(
    TableId,
    "A semantic table identity, separate from object IDs."
);
identity!(
    OperationId,
    "A logical operation identity, separate from record IDs."
);
identity!(
    CursorId,
    "A cursor identity issued by the session that owns it."
);

/// A caller-issued source/session generation that namespaces record locators.
///
/// Owners must issue a new generation when locators become invalid. This is not
/// a transaction revision, wall clock, or claim of backend snapshot isolation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceGeneration(u64);

impl SourceGeneration {
    /// Wrap an owner-issued generation.
    pub const fn new(generation: u64) -> Self {
        Self(generation)
    }

    /// Read the generation token.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// An opaque backend record locator scoped to a table and source generation.
///
/// A backend must supply a genuine locator; a displayed row number or a set of
/// non-unique displayed values is insufficient. Byte interpretation, validity
/// and scope checks belong to the backend. No global durability is implied.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct RecordId {
    table: TableId,
    generation: SourceGeneration,
    token: Vec<u8>,
}

impl RecordId {
    /// Wrap a backend-issued locator. This does not validate native token bytes.
    pub fn new(table: TableId, generation: SourceGeneration, token: Vec<u8>) -> Self {
        Self {
            table,
            generation,
            token,
        }
    }

    /// Return the table that owns this record locator.
    pub const fn table(&self) -> TableId {
        self.table
    }

    /// Return the locator's source generation.
    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    /// Borrow opaque locator bytes for the backend, not for diagnostic display.
    pub fn token(&self) -> &[u8] {
        &self.token
    }
}

impl fmt::Debug for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordId")
            .field("table", &self.table)
            .field("generation", &self.generation)
            .field("token", &"[redacted]")
            .finish()
    }
}

/// A position token scoped to a cursor and its source generation.
///
/// It does not become a record key even if a backend uses the same token bytes.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct CursorBookmark {
    cursor: CursorId,
    generation: SourceGeneration,
    token: Vec<u8>,
}

impl CursorBookmark {
    /// Wrap a cursor-issued bookmark; the cursor owns its validity rules.
    pub fn new(cursor: CursorId, generation: SourceGeneration, token: Vec<u8>) -> Self {
        Self {
            cursor,
            generation,
            token,
        }
    }

    /// Return the owning cursor identity.
    pub const fn cursor(&self) -> CursorId {
        self.cursor
    }

    /// Return the bookmark's source generation.
    pub const fn generation(&self) -> SourceGeneration {
        self.generation
    }

    /// Borrow the opaque bookmark bytes.
    pub fn token(&self) -> &[u8] {
        &self.token
    }
}

impl fmt::Debug for CursorBookmark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CursorBookmark")
            .field("cursor", &self.cursor)
            .field("generation", &self.generation)
            .field("token", &"[redacted]")
            .finish()
    }
}
