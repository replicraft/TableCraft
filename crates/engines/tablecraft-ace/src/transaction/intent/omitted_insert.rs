//! Ordered original supplied/omitted ordinary INSERT, distinct from complete TCNA1.
mod codec;
mod planning;
mod recovery;

#[cfg(test)]
mod tests;

pub use planning::RecoverableNativeOmittedInsertPlan;
pub use recovery::{
    QualifiedNativeOmittedInsertRecovery, QualifiedRetainedNativeOmittedInsertRecovery,
    qualify_native_omitted_insert_recovery,
};

use super::{NativeIntentError, binding, model as names, row_carrier::Profile};
use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceFamily {
    Ordinary,
    Current,
    Retained,
    RetainedOpaque,
    RetainedKeyless,
}

/// Original ordered supplied/omitted rows and an exact complete BEFORE binding.
/// This data has no public conversion into a native plan or absence permission.
pub struct NativeOmittedInsertIntent {
    table: SchemaName,
    rows: Vec<OmittedRowInsert>,
    before_length: u64,
    before_binding: [u8; 32],
    profile: Profile,
    family: SourceFamily,
}

impl NativeOmittedInsertIntent {
    /// Maximum complete TCNA0002 record accepted before host allocation.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Current-presence framing stores the total byte length in a u32.
    pub const MAX_CURRENT_ENCODED_BYTES: usize = u32::MAX as usize;

    /// Retained original presence uses the same total-u32 carrier envelope;
    /// native row/schema capacity is checked independently by its planner.
    pub const MAX_RETAINED_ENCODED_BYTES: usize = u32::MAX as usize;

    /// Validate and retain original requests in their written order.
    /// Actual target eligibility, row width and final constraints are
    /// independently checked by the normal planner, never granted here.
    pub fn new(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(
            table,
            rows,
            before,
            SourceFamily::Ordinary,
            budget,
            cancelled,
        )
    }

    /// Retain current-history original presence in the distinct TCNA0007 family.
    /// This binds data only; it grants no source, default or insertion authority.
    pub fn new_current(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(
            table,
            rows,
            before,
            SourceFamily::Current,
            budget,
            cancelled,
        )
    }

    /// Bind retained surrounding storage to TCNA0008 without granting mutation.
    pub fn new_retained(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(
            table,
            rows,
            before,
            SourceFamily::Retained,
            budget,
            cancelled,
        )
    }

    /// Bind opaque retained history to TCNA0009 without granting insertion.
    pub fn new_retained_opaque(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(
            table,
            rows,
            before,
            SourceFamily::RetainedOpaque,
            budget,
            cancelled,
        )
    }

    /// Bind original ordinary presence to TCNA0010. This grants no source,
    /// literal-default, generated-value or insertion authority.
    ///
    /// # Errors
    /// Refuses invalid names, carriers or native image lengths, cancellation,
    /// allocation failure and explicit resource exhaustion.
    pub fn new_retained_keyless(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::capture(
            table,
            rows,
            before,
            SourceFamily::RetainedKeyless,
            budget,
            cancelled,
        )
    }

    /// Exact TCNA0010 protocol identity, never a source certificate.
    pub fn is_retained_keyless(&self) -> bool {
        self.family == SourceFamily::RetainedKeyless
    }

    /// Exact opaque-retained protocol identity, separate from TCNA0008.
    pub fn is_retained_opaque(&self) -> bool {
        self.family == SourceFamily::RetainedOpaque
    }

    /// Exact decoded protocol identity, never source or apply authority.
    pub fn is_retained(&self) -> bool {
        self.family == SourceFamily::Retained
    }

    fn capture(
        table: &str,
        rows: Vec<OmittedRowInsert>,
        before: &[u8],
        family: SourceFamily,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let _span = tracing::debug_span!("native.omitted_insert_intent.capture").entered();
        check(budget, cancelled)?;
        names::validate_name(table, budget, cancelled)?;
        let mut profile = if family != SourceFamily::Ordinary {
            Profile::Wider
        } else {
            Profile::Legacy
        };
        for row in &rows {
            for field in row.values() {
                check(budget, cancelled)?;
                if let crate::InsertFieldValue::Supplied(value) = field
                    && Profile::for_value(value) == Profile::Wider
                {
                    profile = Profile::Wider;
                }
            }
        }
        codec::validate_requests(table.len(), &rows, profile, family, budget, cancelled)?;
        let before_binding = match family {
            SourceFamily::Retained
            | SourceFamily::RetainedOpaque
            | SourceFamily::RetainedKeyless => binding::digest_retained(before, budget, cancelled)?,
            _ => binding::digest(before, budget, cancelled)?,
        };
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            rows,
            before_length: before.len() as u64,
            before_binding,
            profile,
            family,
        })
    }

    /// Legacy envelopes must not acquire the wider supplied-carrier grammar.
    pub(super) fn is_legacy_profile(&self) -> bool {
        self.family == SourceFamily::Ordinary && self.profile == Profile::Legacy
    }

    /// Exact original-presence wire size for a bounded execution-input envelope.
    pub(in crate::transaction::intent) fn encoded_length(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, NativeIntentError> {
        codec::validate_requests(
            self.table().len(),
            self.rows(),
            self.profile,
            self.family,
            budget,
            cancelled,
        )
    }

    fn plan(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        match self.family {
            SourceFamily::Ordinary => image.plan_ordinary_omitted_row_inserts(
                self.table(),
                self.rows(),
                budget,
                cancelled,
            ),
            #[cfg(feature = "ace16-create")]
            SourceFamily::Current => {
                image.plan_current_omitted_row_inserts(self.table(), self.rows(), budget, cancelled)
            }
            #[cfg(feature = "ace16-create")]
            SourceFamily::Retained => image.plan_retained_omitted_row_inserts(
                self.table(),
                self.rows(),
                budget,
                cancelled,
            ),
            #[cfg(feature = "ace16-create")]
            SourceFamily::RetainedOpaque => image.plan_retained_opaque_omitted_row_inserts(
                self.table(),
                self.rows(),
                budget,
                cancelled,
            ),
            #[cfg(feature = "ace16-create")]
            SourceFamily::RetainedKeyless => image.plan_retained_keyless_omitted_row_inserts(
                self.table(),
                self.rows(),
                budget,
                cancelled,
            ),
            #[cfg(not(feature = "ace16-create"))]
            SourceFamily::Current
            | SourceFamily::Retained
            | SourceFamily::RetainedOpaque
            | SourceFamily::RetainedKeyless => {
                Err(EditError::Unsupported(crate::WriteBlocker::RowLayout))
            }
        }
    }

    /// Canonical caller-supplied table name without implicit normalization.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact original presence rows, including an empty batch and written order.
    pub fn rows(&self) -> &[OmittedRowInsert] {
        &self.rows
    }

    /// Verify exact source bytes before any native replay or target effects.
    pub fn verify_before(
        &self,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeIntentError> {
        check(budget, cancelled)?;
        if before.len() as u64 != self.before_length {
            return Err(NativeIntentError::BeforeMismatch);
        }
        let actual = match self.family {
            SourceFamily::Retained
            | SourceFamily::RetainedOpaque
            | SourceFamily::RetainedKeyless => binding::digest_retained(before, budget, cancelled)?,
            _ => binding::digest(before, budget, cancelled)?,
        };
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if !tablecraft_crypto::constant_time_eq(&actual, &self.before_binding) {
            return Err(NativeIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

#[cfg(test)]
mod current_tests;

#[cfg(test)]
mod retained_tests;

#[cfg(test)]
mod opaque_tests;
