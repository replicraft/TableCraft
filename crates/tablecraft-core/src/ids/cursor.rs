//! Cursor bookmarks with explicit owner and generation scope.

use std::fmt;

use super::{CursorId, SourceGeneration};

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
