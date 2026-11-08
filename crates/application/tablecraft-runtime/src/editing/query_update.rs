//! Lease-free SQL UPDATE review owns the actual native plan, never UI authority.
use super::EditError;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use super::ids::issue_id;
use crate::{Error, InspectedQuery, Response, document::Document};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use crate::{
    controls::{SCOPE, check, comparison_budget, update_query_budget, update_review_budget},
    display::Presentation,
};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use std::{
    cell::RefCell,
    sync::{Arc, atomic::AtomicBool},
};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_access::{NativeSource, editing::sql};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_ace::{InspectError, NativeImage, RecoverableNativeEditPlan};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_core::limits::Resource;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_query::UpdatePlan;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use tablecraft_sql::{ParseLimits, parse_query};

/// Complete reviewed counts and display facts, not permission to write.
pub struct QueryUpdateReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub matched_rows: usize,
    pub changed_rows: usize,
}

/// Runtime-issued source-bound UPDATE, consumed by the nonreplaceable save worker.
/// Its native plan and invalidation token cannot be constructed by a host.
pub struct ReviewedQueryUpdate {
    pub(super) state: Box<QueryUpdateState>,
}

pub(super) struct QueryUpdateState {
    pub(super) path: PathBuf,
    pub(super) document_revision: u64,
    pub(super) table: String,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) plan: UpdatePlan<RecoverableNativeEditPlan>,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    pub(super) valid: Arc<AtomicBool>,
}

impl ReviewedQueryUpdate {
    pub fn source_path(&self) -> &Path {
        &self.state.path
    }

    pub fn table(&self) -> &str {
        &self.state.table
    }

    pub fn document_revision(&self) -> u64 {
        self.state.document_revision
    }
}

impl Document {
    pub(crate) fn review_query_update(
        &mut self,
        target: &InspectedQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        self.require_query_inspection(target, cancelled)?;
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            Err(EditError::UnsupportedPlatform.into())
        }
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            let mut query_work = update_query_budget();
            query_work.charge(Resource::Bytes, target.sql().len() as u64)?;
            query_work.charge(Resource::Steps, target.sql().len() as u64)?;
            let parsed = parse_query(target.sql(), ParseLimits::default())?;
            let table = sql::local_batch_update_table(parsed.query())
                .map_err(sql::RecoverableError::Shape)?;
            if !self.source.editable_native() {
                return Err(EditError::ReadOnlySource.into());
            }

            let native_pool = RefCell::new(update_review_budget(self.edits.original.len())?);
            let image = {
                let mut native = native_pool.borrow_mut();
                NativeImage::capture(&mut self.source, SCOPE, &mut native, cancelled)?
            };
            let source = NativeSource::new_with_shared_budget(
                &mut self.source,
                SCOPE,
                &native_pool,
                cancelled,
            )?;
            let comparison_pool = RefCell::new(comparison_budget());
            let (_source, resolver) =
                source.with_deferred_database_collation_resolver(&comparison_pool)?;

            let plan = {
                let mut native = native_pool
                    .try_borrow_mut()
                    .map_err(|_| Error::Native(InspectError::Unsupported))?;
                sql::plan_recoverable_batch_update_with_comparison(
                    image,
                    parsed.query(),
                    &mut native,
                    &mut query_work,
                    resolver.default_collation(),
                    &resolver,
                    cancelled,
                )?
            };
            check(cancelled)?;

            let mut output = Presentation::new(cancelled);
            let review_table = output.text(table)?;
            let capability_table = output.text(table)?;
            let matched_rows = plan.matched_rows();
            let changed_rows = plan.backend().changed_rows();
            {
                let mut native = native_pool
                    .try_borrow_mut()
                    .map_err(|_| Error::Native(InspectError::Unsupported))?;
                native.charge_allocation(self.edits.path.as_os_str().len() as u64, 1)?;
                native.charge_allocation(1, size_of::<QueryUpdateState>() as u64)?;
            }
            let capability = ReviewedQueryUpdate {
                state: Box::new(QueryUpdateState {
                    path: self.edits.path.clone(),
                    document_revision: self.revision,
                    table: capability_table,
                    plan,
                    valid: Arc::clone(&self.edits.valid),
                }),
            };
            let review = QueryUpdateReview {
                id: issue_id()?,
                document_revision: self.revision,
                table: review_table,
                matched_rows,
                changed_rows,
            };
            check(cancelled)?;
            Ok(Response::QueryUpdateReviewed { review, capability })
        }
    }
}
