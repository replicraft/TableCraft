//! Explicit normal source selection, separate from frozen TCAC1–5 entrypoints.
use super::{
    CreatedFieldAdditionSourcePurpose, FieldAdditionAfterOffError,
    FieldAdditionAfterOffHistoryError, FieldAdditionError, FieldAdditionTarget, after_off,
};
use crate::{
    CurrentFieldAdditionError, FieldHistoryDatabaseOptionsError, FileFormat, InspectError,
    NativeImage,
    format::{
        page::PAGE_BYTES,
        row::{reference_page, reference_slot},
    },
    header::check,
    inspect_table_schema,
    properties::database,
    table::mutation::catalog_property::ledger,
};
use tablecraft_core::limits::ResourceBudget;
use thiserror::Error;

const POST_OFF_OBJECTS: usize = crate::creation::profile::CATALOG_OBJECT_COUNT + 1;

/// Metadata-selected source purpose, never proof or mutation permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionSourcePurpose {
    /// Current metadata with explicit OFF, preserving existing rows and histories.
    CurrentNumber,
    /// Existing explicit first/repeated/typed language and its unchanged proof.
    Created(CreatedFieldAdditionSourcePurpose),
    /// Actual native database role; only full settled OFF proof can admit it.
    AfterExplicitOff,
    /// Actual user and DB Single references place the user later on one shared page.
    /// Full chronological source proof and fitted capacity are still required.
    AfterExplicitOffHistory,
}

/// Selected source refusal while preserving inspection/resource identity.
#[derive(Debug, Error)]
pub enum FieldAdditionSourceError {
    /// Current metadata ownership, dependencies or measured fitting failed.
    #[error(transparent)]
    Current(#[from] CurrentFieldAdditionError),
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
    /// Complete post-OFF history or capacity failed outside ordinary source refusal.
    #[error(transparent)]
    History(FieldAdditionAfterOffHistoryError),
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

impl From<FieldAdditionAfterOffHistoryError> for FieldAdditionSourceError {
    fn from(error: FieldAdditionAfterOffHistoryError) -> Self {
        match error {
            FieldAdditionAfterOffHistoryError::Inspect(error) => Self::Inspect(error),
            FieldAdditionAfterOffHistoryError::Source
            | FieldAdditionAfterOffHistoryError::Field(FieldAdditionAfterOffError::Namespace)
            | FieldAdditionAfterOffHistoryError::Field(FieldAdditionAfterOffError::Field(
                FieldAdditionError::Source,
            ))
            | FieldAdditionAfterOffHistoryError::Field(FieldAdditionAfterOffError::SourceProof(
                FieldHistoryDatabaseOptionsError::Source,
            ))
            | FieldAdditionAfterOffHistoryError::Field(FieldAdditionAfterOffError::SourceProof(
                FieldHistoryDatabaseOptionsError::Field(FieldAdditionError::Source),
            )) => Self::Source,
            error => Self::History(error),
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

    /// Measured maximum ASCII name units for OFF/history branches; None on legacy sources.
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
        if let Some(database) = database::role(self.catalog(), budget, cancelled)? {
            if matches!(target, FieldAdditionTarget::NumberLongZero) {
                return Ok(FieldAdditionSourcePurpose::CurrentNumber);
            }
            if table.len() > after_off::MAX_ASCII_NAME_BYTES {
                return Err(FieldAdditionSourceError::Source);
            }
            budget
                .charge(tablecraft_core::limits::Resource::Steps, table.len() as u64)
                .map_err(InspectError::from)?;
            if !table.is_ascii() {
                return Err(FieldAdditionSourceError::Source);
            }
            // Both selected OFF languages require the same nine native roles.
            // Preserve known ineligible catalogs before specialized column maps.
            if self.catalog().origins().len() != POST_OFF_OBJECTS {
                return Ok(FieldAdditionSourcePurpose::AfterExplicitOff);
            }
            let schema = inspect_table_schema(
                &mut self.reader(),
                self.catalog().catalog().scope(),
                table,
                budget,
                cancelled,
            )?;
            let live = ledger::capture(self.as_bytes(), self.catalog(), budget, cancelled)
                .map_err(FieldAdditionAfterOffError::from)?;
            let user = live.single_reference(schema.object_id(), budget, cancelled)?;
            let database = live.single_reference(database.object_id(), budget, cancelled)?;
            if let (Some((user, _)), Some((database, _))) = (user, database)
                && reference_page(user) == reference_page(database)
                && reference_slot(user) > reference_slot(database)
            {
                return Ok(FieldAdditionSourcePurpose::AfterExplicitOffHistory);
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
            FieldAdditionSourcePurpose::CurrentNumber => {
                let number =
                    self.inspect_current_field_addition_source(table, budget, cancelled)?;
                let number = u8::try_from(number.maximum_name_utf16_units())
                    .map_err(|_| FieldAdditionSourceError::Source)?;
                let text_purpose = self.field_addition_source_purpose(
                    table,
                    FieldAdditionTarget::ShortText {
                        max_utf16_units: 255,
                        unicode_compression: true,
                    },
                    budget,
                    cancelled,
                )?;
                let text = match text_purpose {
                    FieldAdditionSourcePurpose::AfterExplicitOff => {
                        after_off::availability::inspect(self, table, budget, cancelled)
                            .map_err(FieldAdditionSourceError::from)
                    }
                    FieldAdditionSourcePurpose::AfterExplicitOffHistory => {
                        after_off::availability::inspect_history(self, table, budget, cancelled)
                            .map_err(FieldAdditionSourceError::from)
                    }
                    _ => return Err(FieldAdditionSourceError::Source),
                };
                let text = match text {
                    Ok(limits) => limits.1,
                    Err(FieldAdditionSourceError::Source) => None,
                    Err(error) => return Err(error),
                };
                Some(((number != 0).then_some(number), text))
            }
            FieldAdditionSourcePurpose::Created(_) => {
                self.check_created_field_addition_source(table, budget, cancelled)?;
                None
            }
            FieldAdditionSourcePurpose::AfterExplicitOff => Some(after_off::availability::inspect(
                self, table, budget, cancelled,
            )?),
            FieldAdditionSourcePurpose::AfterExplicitOffHistory => Some(
                after_off::availability::inspect_history(self, table, budget, cancelled)?,
            ),
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
