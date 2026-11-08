//! Source-bound existing-query review, independent of rows and schema authoring.
use super::platform;
use crate::{Error, Response, SavedQueryTarget, document::Document};
use bytes::Bytes;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Exact original/proposed literals and document-owned query selected for review.
pub struct QueryExpressionReview {
    pub id: u64,
    pub target: SavedQueryTarget,
    pub native_id: i32,
    pub expected: String,
    pub replacement: String,
    pub changed: bool,
}

/// Complete expected content for read-only reopening after an acknowledged save.
/// This witness cannot save, execute or replay the original query request.
pub struct QueryExpressionReopen {
    pub(crate) path: PathBuf,
    pub(crate) expected: Bytes,
    pub(crate) target: SavedQueryTarget,
    pub(crate) native_id: i32,
}

impl QueryExpressionReopen {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    pub fn query_name(&self) -> &str {
        &self.target.name
    }

    pub fn native_id(&self) -> i32 {
        self.native_id
    }

    pub fn document_revision(&self) -> u64 {
        self.target.document_revision
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

/// Only a successful runtime/native review issues this consuming capability.
pub struct ReviewedQueryExpression {
    pub(super) state: Box<platform::query_expression::State>,
}

impl ReviewedQueryExpression {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<QueryExpressionReopen> {
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_query_literal_expression(
        &mut self,
        target: &SavedQueryTarget,
        expected: &str,
        replacement: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if target.document_revision != self.revision {
            return Err(Error::StaleSavedQuery);
        }
        platform::query_expression::review(self, target, expected, replacement, cancelled)
    }
}
