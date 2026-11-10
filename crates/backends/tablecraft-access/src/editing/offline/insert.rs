//! Complete-row typed and SQL INSERT over the same retained target lease.
mod batch;
mod complete_batch;

use super::plan::MutationPlan;
use super::table::require_original;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SessionState};
use crate::editing::{
    journal::{NativeJournalInsertPlan, NativeJournalInserter},
    sql,
};
use std::path::Path;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_schema::{Column, SchemaPolicy};
use tablecraft_sql::query::Query;

/// Metadata-only complete supplied-column schema retaining the target owner.
/// No row handles or generated-column companions are issued. Ordinary omitted
/// fields retain presence until fresh native planning and recovery.
pub struct OfflineTableInserter {
    session: OfflineWriteSession,
    inserter: NativeJournalInserter,
}

/// Qualified insertion with its count available before durable effects.
pub struct StagedInsert {
    pub(super) session: OfflineWriteSession,
    pub(super) plan: NativeJournalInsertPlan,
}

impl StagedInsert {
    /// Number of rows inserted by the qualified native plan.
    pub fn inserted_rows(&self) -> usize {
        self.plan.inserted_rows()
    }

    /// Abandon this unprepared action without releasing the target lease.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Commit once through the existing retained namespace lifecycle.
    /// INSERT receipts have no matched-row count.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let inserted = self.plan.inserted_rows();
        super::apply::apply(
            self.session,
            MutationPlan::Insert(self.plan),
            super::apply::CompletionFacts::Rows {
                matched: None,
                changed: Some(inserted),
            },
            bundle,
            budget,
            cancelled,
        )
    }
}

impl OfflineWriteSession {
    /// Inspect the complete insertion schema from the SAME retained descriptor.
    /// This does not scan target rows or create a journal.
    pub fn insert_table(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableInserter, Failure> {
        self.insert_table_with_original(table, None, budget, cancelled)
    }

    /// Require the complete reviewed image before native insertion metadata is
    /// constructed. Equality and the inserter use the same exclusively retained
    /// descriptor capture; a mismatch creates no journal and retains its owner.
    pub fn insert_table_if_original(
        self,
        table: &str,
        expected: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableInserter, Failure> {
        self.insert_table_with_original(table, Some(expected), budget, cancelled)
    }

    fn insert_table_with_original(
        mut self,
        table: &str,
        expected: Option<&[u8]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableInserter, Failure> {
        let _span = tracing::debug_span!("access.offline.insert_table").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalInserter::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
                budget,
                cancelled,
            )
            .map_err(Cause::Editor)?;
            slot.state = SessionState::Ready;
            if let Some(expected) = expected {
                require_original(image.as_bytes(), expected, budget, cancelled)?;
            }
            NativeJournalInserter::new(image, table, budget, cancelled).map_err(Cause::Editor)
        })();
        match result {
            Ok(inserter) => Ok(OfflineTableInserter {
                session: self,
                inserter,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }

    /// Bind one local VALUES row; every supplied carrier is retained in TCNA.
    pub fn plan_insert(
        mut self,
        query: &Query,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedInsert, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_insert").entered();
        let result = (|| {
            self.require_ready()?;
            let table =
                sql::local_insert_table(query, false, budget, cancelled).map_err(|error| {
                    Cause::InsertPlanning(sql::RecoverableInsertError::Shape(error))
                })?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalInserter::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
                budget,
                cancelled,
            )
            .map_err(Cause::Editor)?;
            slot.state = SessionState::Ready;
            sql::plan_recoverable_insert(image, query, budget, cancelled)
                .map_err(Cause::InsertPlanning)
        })();
        match result {
            Ok(plan) => Ok(StagedInsert {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }

    /// Explicit clock capability for original omitted defaults, sampled only
    /// after fresh native binding and retained unchanged for recovery.
    pub fn plan_insert_with_clock(
        mut self,
        query: &Query,
        provider: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedInsert, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_insert").entered();
        let result = (|| {
            self.require_ready()?;
            let table =
                sql::local_insert_table(query, false, budget, cancelled).map_err(|error| {
                    Cause::InsertPlanning(sql::RecoverableInsertError::Shape(error))
                })?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalInserter::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
                budget,
                cancelled,
            )
            .map_err(Cause::Editor)?;
            slot.state = SessionState::Ready;
            sql::plan_recoverable_insert_with_clock(image, query, provider, budget, cancelled)
                .map_err(Cause::InsertPlanning)
        })();
        match result {
            Ok(plan) => Ok(StagedInsert {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl OfflineTableInserter {
    /// Abandon metadata preparation and retain the exclusive target lease.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }
}

impl InsertableTable for OfflineTableInserter {
    type Error = Failure;
    type Plan = StagedInsert;

    fn name(&self) -> &str {
        self.inserter.name()
    }

    fn columns(&self) -> &[Column] {
        self.inserter.columns()
    }

    fn schema_policy(&self) -> SchemaPolicy {
        self.inserter.schema_policy()
    }

    fn requires_field_list(&self) -> bool {
        self.inserter.requires_field_list()
    }

    fn plan_insert(
        self,
        values: Vec<Value>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.offline.plan_typed_insert").entered();
        let Self { session, inserter } = self;
        match inserter.plan_insert(values, budget, cancelled) {
            Ok(plan) => Ok(StagedInsert { session, plan }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}

impl OmittingInsertableTable for OfflineTableInserter {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let _span = tracing::debug_span!("access.offline.plan_omitted_insert").entered();
        let Self { session, inserter } = self;
        match inserter.plan_insert_with_omissions(values, budget, cancelled) {
            Ok(plan) => Ok(StagedInsert { session, plan }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}

impl OfflineTableInserter {
    /// Typed original presence with a host clock capability, not native permission.
    pub fn plan_insert_with_omissions_and_clock(
        self,
        values: Vec<InsertValue>,
        provider: &dyn Clock,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedInsert, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_omitted_insert").entered();
        let Self { session, inserter } = self;
        match inserter.plan_insert_with_omissions_and_clock(values, provider, budget, cancelled) {
            Ok(plan) => Ok(StagedInsert { session, plan }),
            Err(error) => Err(session.fail(Phase::Planning, Cause::Editing(error))),
        }
    }
}
