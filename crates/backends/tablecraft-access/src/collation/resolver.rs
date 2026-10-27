//! Exact native comparison descriptor resolution over one shared quota.
use super::{FrenchLegacyCollation, GeneralCollation};
use std::cell::RefCell;
use tablecraft_ace::{GeneralEqualityPrefix, InspectError, NativeSortOrder, TextIndexSelection};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode},
};
use tablecraft_query::{
    QueryError,
    source::{CollationResolver, TextCollationId, TextEqualityKey},
};

const LEGACY_ORDER: [u8; 4] = [9, 4, 0, 0];
const GENERAL_2010_ORDER: [u8; 4] = [9, 4, 0, 1];
const FRENCH_LEGACY_ORDER: [u8; 4] = [12, 4, 0, 0];
const _: () = assert!(GeneralEqualityPrefix::MAX_BYTES <= TextEqualityKey::MAX_BYTES);

/// Native General legacy/2010 and comparison-only French legacy policies.
/// All policies share one caller-owned cumulative quota.
/// Exact unknown identities fail; no ordinal/default fallback or OS locale is
/// inferred. The weight/lineage models remain provisional without native SQL qualification.
/// Hosts own the budget cell and cancellation lifetime; reentrant borrowing fails
/// explicitly. No heap-owned cache, runtime or source discovery is introduced.
pub struct NativeCollationResolver<'a> {
    default: NativeSortOrder,
    legacy: GeneralCollation<'a>,
    modern: GeneralCollation<'a>,
    french: FrenchLegacyCollation<'a>,
}
impl<'a> NativeCollationResolver<'a> {
    /// Validate the stored default descriptor and construct the exact policies.
    /// Construction, resolution, comparisons, searches and LIKE callbacks consume
    /// the same cumulative caller-owned quota across families and executions.
    pub fn new(
        default: NativeSortOrder,
        budget: &'a RefCell<ResourceBudget>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        if default.general_family().is_none()
            && default != NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER)
        {
            return Err(InspectError::Unsupported);
        }
        let resolver = Self {
            default,
            legacy: GeneralCollation::new_shared(
                NativeSortOrder::from_native_bytes(LEGACY_ORDER),
                budget,
                cancelled,
            )?,
            modern: GeneralCollation::new_shared(
                NativeSortOrder::from_native_bytes(GENERAL_2010_ORDER),
                budget,
                cancelled,
            )?,
            french: FrenchLegacyCollation::new_shared(
                NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
                budget,
                cancelled,
            )?,
        };
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        Ok(resolver)
    }
    /// Explicit host module/default string mode; never inferred from file content.
    pub fn with_option_compare(mut self, mode: StringCompareMode) -> Self {
        self.legacy = self.legacy.with_option_compare(mode);
        self.modern = self.modern.with_option_compare(mode);
        self.french = self.french.with_option_compare(mode);
        self
    }
    /// Borrow the configured database/default policy; unknown defaults never
    /// reach construction and the descriptor is not inferred from file generation.
    /// The trait return replaces the previous GeneralCollation reference so a
    /// French default retains its exact provider instead of a General fallback.
    pub fn default_collation(&self) -> &dyn Collation {
        if self.default == self.legacy.order {
            &self.legacy
        } else if self.default == self.modern.order {
            &self.modern
        } else {
            &self.french
        }
    }
}
impl CollationResolver for NativeCollationResolver<'_> {
    fn equality_key(
        &self,
        key: TextCollationId,
        literal: &str,
    ) -> Result<Option<TextEqualityKey>, QueryError> {
        let policy = if key == identity(self.legacy.order) {
            &self.legacy
        } else if key == identity(self.modern.order) {
            &self.modern
        } else {
            // A hint must not activate an unknown comparison on an empty source.
            return Ok(None);
        };
        if (policy.cancelled)() {
            return Err(EvalError::Cancelled.into());
        }
        let mut budget = policy
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        for (position, _) in literal.encode_utf16().enumerate() {
            if (policy.cancelled)() {
                return Err(EvalError::Cancelled.into());
            }
            budget.charge(Resource::Steps, 1)?;
            if position >= TextIndexSelection::MAX_BOUND_UNITS {
                return Ok(None);
            }
        }
        let prefix =
            GeneralEqualityPrefix::from_text(policy.order, literal, &mut budget, policy.cancelled)
                .map_err(super::adapt)?;
        TextEqualityKey::from_host(key, prefix.as_bytes()).map(Some)
    }

    fn resolve(&self, key: TextCollationId) -> Result<&dyn Collation, QueryError> {
        if (self.legacy.cancelled)() {
            return Err(EvalError::Cancelled.into());
        }
        {
            let mut budget = self
                .legacy
                .budget
                .try_borrow_mut()
                .map_err(|_| EvalError::CollationFailed)?;
            budget.charge(Resource::Steps, 1)?;
        }
        let policy: &dyn Collation = if key == identity(self.legacy.order) {
            &self.legacy
        } else if key == identity(self.modern.order) {
            &self.modern
        } else if key == identity(NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER)) {
            &self.french
        } else {
            return Err(EvalError::CollationRequired.into());
        };
        if (self.legacy.cancelled)() {
            return Err(EvalError::Cancelled.into());
        }
        Ok(policy)
    }
}

pub(crate) fn identity(order: NativeSortOrder) -> TextCollationId {
    let locale = order.lcid().to_le_bytes();
    TextCollationId::from_host(u64::from(u32::from_le_bytes([
        locale[0],
        locale[1],
        order.variant(),
        order.version(),
    ])))
}
