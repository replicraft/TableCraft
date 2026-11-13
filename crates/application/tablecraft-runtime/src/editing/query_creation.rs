//! Nominal creation review and immutable query-created publication evidence.
mod outcome;
pub use outcome::{QueryCreationOutcomeError, QueryCreationReopenFacts};

use super::platform;
use crate::{Error, Response, SavedQueryTarget, document::Document};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tablecraft_core::values::LegacyDateTime;
use tablecraft_expressions::functions::dates::clock::Clock;

/// Host correlation facts for the authored draft; these grant no save authority.
#[derive(Debug, PartialEq, Eq)]
pub struct QueryCreationTarget {
    pub document_revision: u64,
    pub draft_revision: u64,
    /// Actual saved editor owner for authored Save As; None is scratch SQL.
    pub source: Option<SavedQueryTarget>,
}

/// Actual source of this new definition, separate from its destination identity.
#[derive(Debug, PartialEq, Eq)]
pub enum QueryCreationOrigin {
    /// Exact submitted authoring text and its caller-owned revision.
    Draft {
        draft_revision: u64,
        sql: String,
        source: Option<SavedQueryTarget>,
    },
    /// Actual stored definition selected by the source document.
    Saved(SavedQueryTarget),
}

/// Completed source-bound review; no affected user rows are reported.
pub struct QueryCreationReview {
    pub id: u64,
    pub document_revision: u64,
    pub origin: QueryCreationOrigin,
    pub destination: String,
    pub native_id: i32,
    pub alias: String,
    pub value: i32,
    pub captured_at: LegacyDateTime,
}

/// Same paid complete AFTER image needed after acknowledged new-query creation.
/// This owner cannot save, execute SQL, replay an original or mint a target.
pub struct QueryCreatedReopen {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) expected: Bytes,
    pub(in crate::editing) document_revision: u64,
    pub(in crate::editing) name: String,
    pub(in crate::editing) native_id: i32,
}

impl QueryCreatedReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub fn query_name(&self) -> &str {
        &self.name
    }

    pub fn native_query_id(&self) -> i32 {
        self.native_id
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Only successful fresh native review constructs this consuming capability.
pub struct ReviewedQueryCreation {
    pub(in crate::editing) state: Box<platform::query_creation::State>,
}

impl ReviewedQueryCreation {
    pub fn id(&self) -> u64 {
        self.state.id
    }

    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<QueryCreatedReopen> {
        // NO_CLONE: The caller retains the immutable full-image witness after
        // the save worker consumes the distinct capability; no payload copy.
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_query_creation(
        &mut self,
        target: &mut QueryCreationTarget,
        sql: &mut String,
        destination: &str,
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        platform::query_creation::review_sql(self, target, sql, destination, clock, cancelled)
    }

    pub(crate) fn review_stored_query_copy(
        &mut self,
        target: &mut SavedQueryTarget,
        destination: &str,
        clock: &dyn Clock,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        platform::query_creation::review_saved(self, target, destination, clock, cancelled)
    }
}
