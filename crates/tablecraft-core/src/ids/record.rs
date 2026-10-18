//! Backend record locators scoped to a table and source generation.

use std::fmt;

use super::{SourceGeneration, TableId};

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
