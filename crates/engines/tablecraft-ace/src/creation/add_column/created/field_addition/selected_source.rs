//! Explicit normal source selection, separate from frozen TCAC1–5 entrypoints.
use super::{
    CreatedFieldAdditionSourcePurpose, FieldAdditionAfterOffError, FieldAdditionError,
    FieldAdditionTarget, after_off,
};
use crate::{
    FieldHistoryDatabaseOptionsError, FileFormat, InspectError, NativeImage,
    format::page::PAGE_BYTES, header::check, properties::database,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

/// Metadata-selected source purpose, never proof or mutation permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionSourcePurpose {
    /// Existing explicit first/repeated/typed language and its unchanged proof.
    Created(CreatedFieldAdditionSourcePurpose),
    /// Actual native database role; only full settled OFF proof can admit it.
    AfterExplicitOff,
}

/// Selected source refusal while preserving inspection/resource identity.
#[derive(Debug, Error)]
pub enum FieldAdditionSourceError {
    /// Native inspection, allocation, cumulative limit or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Known source language or bounded comparison namespace is not qualified.
    #[error("source is outside the selected field-addition language")]
    Source,
    /// Original first/repeated/typed source proof refused outside its source guard.
    #[error(transparent)]
    Created(FieldAdditionError),
    /// Explicit OFF proof or capacity observation refused outside its source guard.
    #[error(transparent)]
    AfterOff(FieldAdditionAfterOffError),
}

impl From<FieldAdditionError> for FieldAdditionSourceError {
    fn from(error: FieldAdditionError) -> Self {
        match error {
            FieldAdditionError::Inspect(error) => Self::Inspect(error),
            FieldAdditionError::Source => Self::Source,
            error => Self::Created(error),
        }
    }
}

impl From<FieldAdditionAfterOffError> for FieldAdditionSourceError {
    fn from(error: FieldAdditionAfterOffError) -> Self {
        match error {
            FieldAdditionAfterOffError::Inspect(error) => Self::Inspect(error),
            FieldAdditionAfterOffError::Namespace
            | FieldAdditionAfterOffError::SourceProof(FieldHistoryDatabaseOptionsError::Source)
            | FieldAdditionAfterOffError::SourceProof(FieldHistoryDatabaseOptionsError::Field(
                FieldAdditionError::Source,
            )) => Self::Source,
            error => Self::AfterOff(error),
        }
    }
}

/// Source facts and optional measured name-width capacity, not named Save permission.
pub struct FieldAdditionSourceAvailability {
    purpose: FieldAdditionSourcePurpose,
    after_off_limits: Option<(Option<u8>, Option<u8>)>,
}

impl FieldAdditionSourceAvailability {
    /// Actual metadata purpose whose complete source proof was selected once.
    pub const fn purpose(&self) -> FieldAdditionSourcePurpose {
        self.purpose
    }

    /// Some valid ASCII field name can fit the new OFF profile, or the old source qualifies.
    pub fn number_available(&self) -> bool {
        self.after_off_limits
            .is_none_or(|limits| limits.0.is_some())
    }

    /// Some valid ASCII field name can fit the new OFF profile, or the old source qualifies.
    pub fn short_text_available(&self) -> bool {
        self.after_off_limits
            .is_none_or(|limits| limits.1.is_some())
    }

    /// Measured maximum ASCII name units for the OFF branch; None on legacy sources.
    /// Inner None means no named field of that declaration fits the fitted page.
    pub const fn after_off_name_limits(&self) -> Option<(Option<u8>, Option<u8>)> {
        self.after_off_limits
    }
}

impl NativeImage {
    /// Select from actual native containment metadata before any source proof.
    /// Existing source-purpose and intent APIs retain their original languages.
    pub fn field_addition_source_purpose(
        &self,
        table: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionSourcePurpose, FieldAdditionSourceError> {
        let _span = tracing::debug_span!("native.field_addition.normal_source_purpose").entered();
        check(budget, cancelled)?;
        self.require_standard_capture()?;
        if self.catalog().header().format() != FileFormat::Ace2016
            || self.catalog().header().page_size() != PAGE_BYTES as u32
        {
            return Err(FieldAdditionSourceError::Source);
        }
        if database::role(self.catalog(), budget, cancelled)?.is_some() {
            if table.len() > after_off::MAX_ASCII_NAME_BYTES {
                return Err(FieldAdditionSourceError::Source);
            }
            budget
                .charge(tablecraft_core::limits::Resource::Steps, table.len() as u64)
                .map_err(InspectError::from)?;
            if !table.is_ascii() {
                return Err(FieldAdditionSourceError::Source);
            }
            return Ok(FieldAdditionSourcePurpose::AfterExplicitOff);
        }
        Ok(FieldAdditionSourcePurpose::Created(
            self.created_field_addition_source_purpose(table, target, budget, cancelled)?,
        ))
    }

    /// Prove exactly one selected source and inspect capacity without producing AFTER.
    /// The consuming named review must still check exact identifier/declaration fit.
    pub fn inspect_field_addition_source(
        self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FieldAdditionSourceAvailability, FieldAdditionSourceError> {
        let _span =
            tracing::debug_span!("native.field_addition.normal_source_inspection").entered();
        let purpose = self.field_addition_source_purpose(
            table,
            FieldAdditionTarget::NumberLongZero,
            budget,
            cancelled,
        )?;
        let after_off_limits = match purpose {
            FieldAdditionSourcePurpose::Created(_) => {
                self.check_created_field_addition_source(table, budget, cancelled)?;
                None
            }
            FieldAdditionSourcePurpose::AfterExplicitOff => Some(after_off::availability::inspect(
                self, table, budget, cancelled,
            )?),
        };
        check(budget, cancelled)?;
        Ok(FieldAdditionSourceAvailability {
            purpose,
            after_off_limits,
        })
    }
}

#[cfg(test)]
mod tests;
