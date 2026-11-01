//! Complete-row typed and SQL INSERT over the same retained target lease.
use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SessionState};
use crate::editing::{journal::NativeJournalInserter, sql};
use std::path::Path;
use tablecraft_ace::{NativeImage, RecoverableNativeInsertPlan};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::InsertableTable;
use tablecraft_schema::{Column, SchemaPolicy};
use tablecraft_sql::query::Query;

/// Metadata-only complete supplied-column schema retaining the target owner.
/// No row handles, omissions, defaults or generated-column companions are issued.
pub struct OfflineTableInserter {
    session: OfflineWriteSession,
    inserter: NativeJournalInserter,
}

/// Qualified insertion with its count available before durable effects.
pub struct StagedInsert {
    session: OfflineWriteSession,
    plan: RecoverableNativeInsertPlan,
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
            (None, inserted),
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
        mut self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<OfflineTableInserter, Failure> {
        let _span = tracing::debug_span!("access.offline.insert_table").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
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
            sql::local_insert_table(query, budget, cancelled).map_err(|error| {
                Cause::InsertPlanning(sql::RecoverableInsertError::Shape(error))
            })?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
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
