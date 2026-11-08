//! Fresh source reconstruction before a consuming column journal can be staged.
use super::{AddColumnIntentError, NativeAddColumnIntent, Profile};
use crate::{
    AddColumnCandidate, CreatedAddColumnCandidate, NativeImage, check_table, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// The actual native constructor purpose is retained before request capture.
pub(super) enum Candidate {
    Keyless(AddColumnCandidate),
    Created(CreatedAddColumnCandidate),
}

impl Candidate {
    pub(super) fn profile(&self) -> Profile {
        match self {
            Self::Keyless(_) => Profile::KeylessV1,
            Self::Created(_) => Profile::CreatedV2,
        }
    }

    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Keyless(plan) => plan.before_bytes(),
            Self::Created(plan) => plan.before_bytes(),
        }
    }

    pub(super) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Keyless(plan) => plan.after_bytes(),
            Self::Created(plan) => plan.after_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::Keyless(plan) => plan.table(),
            Self::Created(plan) => plan.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Keyless(plan) => plan.column(),
            Self::Created(plan) => plan.column(),
        }
    }
}

/// Qualified declaration and original request, without a public arbitrary-AFTER constructor.
pub struct RecoverableAddColumnPlan {
    pub(super) candidate: Candidate,
    pub(super) intent: NativeAddColumnIntent,
}

impl NativeImage {
    /// Select a declared source purpose from actual native index metadata.
    /// Each selected constructor repeats its complete admission; a rejected
    /// constructor is never retried as another profile. Persisted requests later
    /// reconstruct only their captured profile, without this selection step.
    pub fn plan_recoverable_add_int32_column_for_source(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableAddColumnPlan, AddColumnIntentError> {
        check(budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        let checked = check_table(&mut self.reader(), scope, table, budget, cancelled)?;
        if checked.indexes().is_empty() {
            self.plan_recoverable_add_int32_column(table, column, budget, cancelled)
        } else {
            self.plan_recoverable_created_add_int32_column(table, column, budget, cancelled)
        }
    }

    /// Seal the canonical generated-primary source purpose as TCAC0002/profile 2.
    /// The original API and TCAC0001 remain keyless and unmodified.
    pub fn plan_recoverable_created_add_int32_column(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableAddColumnPlan, AddColumnIntentError> {
        let _span = tracing::debug_span!("native.created_add_column.plan_recoverable").entered();
        let candidate = Candidate::Created(
            self.prepare_created_add_int32_column_candidate(table, column, budget, cancelled)?,
        );
        let intent = NativeAddColumnIntent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableAddColumnPlan { candidate, intent })
    }

    /// Qualify an empty-table nullable Int32 addition and bind its original request.
    /// Existing creation, rename and row intent languages are unchanged.
    pub fn plan_recoverable_add_int32_column(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableAddColumnPlan, AddColumnIntentError> {
        let _span = tracing::debug_span!("native.add_column.plan_recoverable").entered();
        let candidate = Candidate::Keyless(
            self.prepare_add_int32_column_candidate(table, column, budget, cancelled)?,
        );
        let intent = NativeAddColumnIntent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableAddColumnPlan { candidate, intent })
    }
}

impl RecoverableAddColumnPlan {
    /// Data-only request describing the checked declaration and complete source.
    pub fn intent(&self) -> &NativeAddColumnIntent {
        &self.intent
    }

    /// Entire captured original; the host retains its exclusive target association.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Entire independently recaptured candidate, not a completion receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Exact selected original table spelling.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Exact proposed appended field spelling.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }

    /// Rebuild from fresh source bytes under separate cumulative native/proof pools.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, AddColumnIntentError> {
        let _span = tracing::debug_span!("native.add_column.requalify").entered();
        same_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate = self.intent.reconstruct(fresh, native, cancelled)?;
        if candidate.profile() != self.candidate.profile() {
            return Err(AddColumnIntentError::ImageMismatch);
        }
        same_image(
            candidate.before_bytes(),
            self.before_bytes(),
            proof,
            cancelled,
        )?;
        same_image(
            candidate.after_bytes(),
            self.after_bytes(),
            proof,
            cancelled,
        )?;
        check(proof, cancelled)?;
        Ok(Self {
            candidate,
            intent: self.intent,
        })
    }

    /// Prepare the same checked V1 image journal used by fixed-length native edits.
    /// Hosts own exclusive offline resources and durable namespace setup/cleanup.
    /// No row counts, filesystem paths or automatic transaction replay are supplied.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.add_column.prepare").entered();
        journal::prepare_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}

pub(super) fn same_image(
    actual: &[u8],
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddColumnIntentError> {
    equal_image(actual, expected, budget, cancelled).map_err(|error| match error {
        crate::NativeRecoveryQualificationError::Inspect(error) => {
            AddColumnIntentError::Inspect(error)
        }
        crate::NativeRecoveryQualificationError::ImageMismatch => {
            AddColumnIntentError::ImageMismatch
        }
        error => AddColumnIntentError::Proof(error),
    })
}
