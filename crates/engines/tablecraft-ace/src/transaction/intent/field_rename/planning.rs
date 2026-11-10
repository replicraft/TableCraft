//! Owning candidate plus original request; no constructor from decoded authority.
use super::{FieldRenameIntentError as Error, NativeFieldRenameIntent as Intent, Profile};
use crate::{
    NativeCurrentFieldRenameCandidate, NativeFieldRenameCandidate, NativeFieldRenameIdentity,
    NativeFittedFieldRenameCandidate, NativeImage, NativePopulatedFieldRenameCandidate,
    NativeRetainedFieldRenameCandidate, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, DurableStorage, JournalError, PreparedImage};

/// Each source-qualified native operation remains sealed behind its own type.
pub(super) enum Candidate {
    SameWidth(NativeFieldRenameCandidate),
    FittedAscii(NativeFittedFieldRenameCandidate),
    PopulatedAscii(NativePopulatedFieldRenameCandidate),
    CurrentAscii(NativeCurrentFieldRenameCandidate),
    RetainedAscii(NativeRetainedFieldRenameCandidate),
}

impl Candidate {
    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::SameWidth(candidate) => candidate.before_bytes(),
            Self::FittedAscii(candidate) => candidate.before_bytes(),
            Self::PopulatedAscii(candidate) => candidate.before_bytes(),
            Self::CurrentAscii(candidate) => candidate.before_bytes(),
            Self::RetainedAscii(candidate) => candidate.before_bytes(),
        }
    }

    pub(super) fn after_bytes(&self) -> &[u8] {
        match self {
            Self::SameWidth(candidate) => candidate.after_bytes(),
            Self::FittedAscii(candidate) => candidate.after_bytes(),
            Self::PopulatedAscii(candidate) => candidate.after_bytes(),
            Self::CurrentAscii(candidate) => candidate.after_bytes(),
            Self::RetainedAscii(candidate) => candidate.after_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::SameWidth(candidate) => candidate.table(),
            Self::FittedAscii(candidate) => candidate.table(),
            Self::PopulatedAscii(candidate) => candidate.table(),
            Self::CurrentAscii(candidate) => candidate.table(),
            Self::RetainedAscii(candidate) => candidate.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::SameWidth(candidate) => candidate.column(),
            Self::FittedAscii(candidate) => candidate.column(),
            Self::PopulatedAscii(candidate) => candidate.column(),
            Self::CurrentAscii(candidate) => candidate.column(),
            Self::RetainedAscii(candidate) => candidate.column(),
        }
    }

    pub(super) fn new_name(&self) -> &str {
        match self {
            Self::SameWidth(candidate) => candidate.new_name(),
            Self::FittedAscii(candidate) => candidate.new_name(),
            Self::PopulatedAscii(candidate) => candidate.new_name(),
            Self::CurrentAscii(candidate) => candidate.new_name(),
            Self::RetainedAscii(candidate) => candidate.new_name(),
        }
    }

    pub(super) fn identity(&self) -> NativeFieldRenameIdentity {
        match self {
            Self::SameWidth(candidate) => candidate.identity(),
            Self::FittedAscii(candidate) => candidate.identity(),
            Self::PopulatedAscii(candidate) => candidate.identity(),
            Self::CurrentAscii(candidate) => candidate.identity(),
            Self::RetainedAscii(candidate) => candidate.identity(),
        }
    }

    pub(super) fn changed(&self) -> bool {
        match self {
            Self::SameWidth(candidate) => candidate.changed(),
            Self::FittedAscii(candidate) => candidate.changed(),
            Self::PopulatedAscii(candidate) => candidate.changed(),
            Self::CurrentAscii(candidate) => candidate.changed(),
            Self::RetainedAscii(candidate) => candidate.changed(),
        }
    }
}

/// Sealed exact field-rename candidate, complete source and original request.
pub struct RecoverableFieldRenamePlan {
    candidate: Box<Candidate>,
    intent: Intent,
}

impl NativeImage {
    /// Bind retained current-storage metadata to a distinct original request.
    /// Fresh source and complete-image reconstruction remain required on recovery.
    pub fn plan_recoverable_retained_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.field_rename.plan_retained").entered();
        let candidate = Candidate::RetainedAscii(
            self.prepare_retained_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(Profile::RetainedAscii, &candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }

    /// Bind a current-metadata rename to the complete original source and request.
    /// This family requalifies actual rows, metadata and dependencies regardless
    /// of earlier renames; it never reinterprets an older serialized profile.
    pub fn plan_recoverable_current_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.field_rename.plan_current").entered();
        let candidate = Candidate::CurrentAscii(
            self.prepare_current_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(Profile::CurrentAscii, &candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }

    /// Bind the populated semantic family to its exact original nonempty source.
    /// The closed profile retains all ASCII widths, including validated no-ops.
    pub fn plan_recoverable_populated_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.field_rename.plan_populated").entered();
        let candidate = Candidate::PopulatedAscii(
            self.prepare_populated_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(Profile::PopulatedAscii, &candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }

    /// Bind the same-width request and original native identity to the whole BEFORE.
    /// This does not open a journal or acquire a native save owner.
    pub fn plan_recoverable_same_width_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span = tracing::debug_span!("native.field_rename.plan_recoverable").entered();
        let candidate = Candidate::SameWidth(
            self.prepare_same_width_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(Profile::SameWidth, &candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }

    /// Capture the fitted ASCII profile separately from the original same-width
    /// profile. Source pages and file length remain unchanged by this operation.
    pub fn plan_recoverable_fitted_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldRenamePlan, Error> {
        let _span =
            tracing::debug_span!("native.field_rename.plan_recoverable_fitted_ascii").entered();
        let candidate = Candidate::FittedAscii(
            self.prepare_fitted_field_rename(table, column, new_name, budget, cancelled)?,
        );
        let intent = Intent::capture(Profile::FittedAscii, &candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        RecoverableFieldRenamePlan::new(candidate, intent, budget, cancelled)
    }
}

impl RecoverableFieldRenamePlan {
    fn new(
        candidate: Candidate,
        intent: Intent,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check(budget, cancelled)?;
        budget.charge_allocation(1, size_of::<Candidate>() as u64)?;

        Ok(Self {
            candidate: Box::new(candidate),
            intent,
        })
    }

    /// Bounded original request data, never serialized storage authority.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete original from the sealed native source proof.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete native candidate reconstructed from the exact request.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Actual original native field spelling.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }

    /// Exact requested spelling, retaining unchanged and case-only distinctions.
    pub fn new_name(&self) -> &str {
        self.candidate.new_name()
    }

    /// Fresh original native identity, including separate number and stored ID.
    pub fn identity(&self) -> NativeFieldRenameIdentity {
        self.candidate.identity()
    }

    /// Fully qualified no-op fact. Hosts may skip persistence for this request.
    pub fn changed(&self) -> bool {
        self.candidate.changed()
    }

    /// Rebuild the original request from fresh bytes under separate cumulative
    /// native and proof pools; no caller-supplied replacement or decoder retry.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.field_rename.requalify").entered();
        equal_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate = self.intent.reconstruct(fresh, native, cancelled)?;
        equal_image(
            candidate.before_bytes(),
            self.before_bytes(),
            proof,
            cancelled,
        )?;
        equal_image(
            candidate.after_bytes(),
            self.after_bytes(),
            proof,
            cancelled,
        )?;
        check(proof, cancelled)?;
        Self::new(candidate, self.intent, native, cancelled)
    }

    /// Prepare the exact equal-length pair in a checked V1 journal using only
    /// caller-owned target/journal resources. Hosts own exclusive association,
    /// durable namespace setup and cleanup; no other journal version is tried.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.field_rename.prepare_v1").entered();
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
