//! Inert native SQL presentation and distinct complete-SQL mutation review.
mod capability;
mod display;
pub use capability::ReviewedQuerySql;

use super::platform;
use crate::{
    Error, Response, SavedQueryTarget,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::{NativeSource, SavedQuerySqlEditError, SavedRead};
use tablecraft_sql::ParseLimits;
use thiserror::Error;

/// A read-only eligibility observation, never authority to mutate or execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuerySqlEligibility {
    /// Fresh native physical qualification admits existing SELECT expression edits.
    QualifiedExpressionSelect,
    /// The definition, storage, source or host is outside the current writer.
    Unavailable,
}

/// Source-bound reconstructed SQL, with independent native write eligibility.
pub struct QuerySqlDraft {
    /// Document-owned target whose read syntax was reconstructed.
    pub target: SavedQueryTarget,
    /// Canonical readable syntax, not original stored bytes or a capability.
    pub sql: Option<String>,
    /// Independent native write-profile observation, never a capability.
    pub eligibility: QuerySqlEligibility,
}

/// Complete submitted SQL and actual document-owned native query review facts.
pub struct QuerySqlReview {
    /// Process-issued identity of this exact consuming review.
    pub id: u64,
    /// Exact captured document/query selected for this review.
    pub target: SavedQueryTarget,
    /// Signed persistent native query identity, not a table/page number.
    pub native_id: i32,
    /// Whether actual native expression bodies change; formatting alone does not.
    pub changed: bool,
    /// Exact original editor text, whose owned syntax matches the actual definition.
    pub original_sql: String,
    /// Exact complete submitted text, without native fragment interpolation.
    pub proposed_sql: String,
}

/// Refusal before complete SQL can acquire its distinct native save capability.
#[derive(Debug, Error)]
pub enum QuerySqlEditError {
    /// Non-expression syntax or a changed expression is outside the profile.
    #[error("saved SQL changes unsupported topology or expression syntax")]
    Unsupported,
    /// Complete original owned SQL differs from the actual captured definition.
    #[error("original saved query SQL no longer matches its definition")]
    OriginalChanged,
    /// Exact metadata/native/source/proof refusal from the owning Access bridge.
    #[error(transparent)]
    Backend(SavedQuerySqlEditError),
}

impl From<SavedQuerySqlEditError> for QuerySqlEditError {
    fn from(error: SavedQuerySqlEditError) -> Self {
        match error {
            SavedQuerySqlEditError::Unsupported => Self::Unsupported,
            SavedQuerySqlEditError::OriginalChanged => Self::OriginalChanged,
            other => Self::Backend(other),
        }
    }
}

impl Document {
    pub(crate) fn inspect_query_sql_draft(
        &mut self,
        target: &SavedQueryTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let saved = self.read_query_sql_target(target, cancelled)?;
        let mut output = Presentation::new(cancelled);
        let sql = display::query(saved.query(), &mut output)?;
        let eligibility = platform::query_sql::eligibility(self, target, &sql, cancelled)?;
        check(cancelled)?;
        Ok(Response::QuerySqlDraft {
            draft: QuerySqlDraft {
                target: SavedQueryTarget {
                    document_revision: target.document_revision,
                    object_id: target.object_id,
                    name: output.text(&target.name)?,
                },
                sql: Some(sql),
                eligibility,
            },
        })
    }

    pub(crate) fn review_query_sql(
        &mut self,
        target: &SavedQueryTarget,
        original_sql: &str,
        proposed_sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleSavedQuery);
        }
        platform::query_sql::review(self, target, original_sql, proposed_sql, cancelled)
    }

    fn read_query_sql_target(
        &mut self,
        target: &SavedQueryTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<SavedRead, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleSavedQuery);
        }
        let mut source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let saved = source.read_saved_query(&target.name, ParseLimits::default())?;
        let object = saved.definition().object();
        if object.id().as_u128() != target.object_id || object.name().as_str() != target.name {
            return Err(Error::StaleSavedQuery);
        }
        Ok(saved)
    }
}
