//! Original declaration and source binding; serialized data grants no native authority.
use super::{
    super::{NativeIntentError, binding},
    candidate::Candidate,
    codec,
};
use crate::{
    CurrentFieldAdditionError, FieldAdditionTarget, InspectError, NativeImage, header::check,
};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_schema::SchemaName;
use thiserror::Error;

pub(super) const BINDING_BYTES: usize = binding::DIGEST_BYTES;
const MAX_NAME_UNITS: usize = 64;
const MAX_NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();

/// Refusal before a typed field-addition request acquires persistence authority.
#[derive(Debug, Error)]
pub enum CurrentFieldAdditionIntentError {
    /// Native inspection, cumulative limits, allocation or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The complete original image could not be bound.
    #[error(transparent)]
    Binding(#[from] NativeIntentError),
    /// The separate current metadata/ownership and fixed declaration proof refused.
    #[error(transparent)]
    Field(#[from] CurrentFieldAdditionError),
    /// Wrong language/profile/declaration, malformed framing or invalid names.
    #[error("invalid typed field-addition intent record")]
    InvalidRecord,
    /// Original complete length or digest differs from the captured source.
    #[error("typed field-addition before image does not match")]
    BeforeMismatch,
    /// Fresh source-resolved table or requested field identity differs.
    #[error("typed field-addition identity does not match")]
    IdentityMismatch,
    /// Complete image comparison failed or exhausted its own proof allowance.
    #[error(transparent)]
    Proof(#[from] crate::NativeRecoveryQualificationError),
    /// Reconstructed complete images differ from the owning candidate/journal.
    #[error("typed field-addition reconstruction differs")]
    ImageMismatch,
}

impl From<LimitError> for CurrentFieldAdditionIntentError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

/// Explicit TCAC0008 Number or TCAC0009 Short Text original request.
/// Number is nullable LONG with default zero; Short Text is nullable with
/// AllowZeroLength(true), explicit width/compression and no default.
/// The original complete image binds every native origin and field identity.
/// Actual current ownership and complete candidate images are requalified.
pub struct NativeCurrentFieldAdditionIntent {
    pub(super) retained: bool,
    pub(super) target: FieldAdditionTarget,
    pub(super) table: SchemaName,
    pub(super) column: SchemaName,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; BINDING_BYTES],
}

impl NativeCurrentFieldAdditionIntent {
    /// Bound checked before a persisted request buffer or names are retained.
    pub const MAX_ENCODED_BYTES: usize = codec::HEADER_BYTES + 2 * MAX_NAME_BYTES;

    pub(super) fn capture(
        candidate: &Candidate,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        validate_column(candidate.column(), budget, cancelled)?;
        Ok(Self {
            retained: candidate.is_retained(),
            target: candidate.target(),
            table: copy_name(candidate.table(), budget, cancelled)?,
            column: copy_name(candidate.column(), budget, cancelled)?,
            before_length: candidate.before_bytes().len() as u64,
            before_binding: if candidate.is_retained() {
                binding::digest_retained(candidate.before_bytes(), budget, cancelled)?
            } else {
                binding::digest(candidate.before_bytes(), budget, cancelled)?
            },
        })
    }

    pub(super) fn reconstruct(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Candidate, CurrentFieldAdditionIntentError> {
        if self.retained {
            return self.reconstruct_retained(image, budget, cancelled);
        }
        let candidate = match self.target {
            FieldAdditionTarget::NumberLongZero => {
                Candidate::Number(image.prepare_current_number_field_addition(
                    self.table(),
                    self.column(),
                    budget,
                    cancelled,
                )?)
            }
            FieldAdditionTarget::ShortText {
                max_utf16_units,
                unicode_compression,
            } => Candidate::ShortText(image.prepare_current_short_text_field_addition(
                self.table(),
                self.column(),
                max_utf16_units,
                unicode_compression,
                budget,
                cancelled,
            )?),
        };
        if candidate.table() != self.table() || candidate.column() != self.column() {
            return Err(CurrentFieldAdditionIntentError::IdentityMismatch);
        }
        Ok(candidate)
    }

    fn reconstruct_retained(
        &self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Candidate, CurrentFieldAdditionIntentError> {
        #[cfg(feature = "ace16-create")]
        {
            let candidate = match self.target {
                FieldAdditionTarget::NumberLongZero => {
                    Candidate::Number(image.prepare_retained_number_field_addition(
                        self.table(),
                        self.column(),
                        budget,
                        cancelled,
                    )?)
                }
                FieldAdditionTarget::ShortText {
                    max_utf16_units,
                    unicode_compression,
                } => Candidate::ShortText(image.prepare_retained_short_text_field_addition(
                    self.table(),
                    self.column(),
                    max_utf16_units,
                    unicode_compression,
                    budget,
                    cancelled,
                )?),
            };
            if candidate.table() != self.table()
                || candidate.column() != self.column()
                || !candidate.is_retained()
            {
                return Err(CurrentFieldAdditionIntentError::IdentityMismatch);
            }
            Ok(candidate)
        }
        #[cfg(not(feature = "ace16-create"))]
        {
            let _ = (image, budget, cancelled);
            Err(CurrentFieldAdditionError::Source.into())
        }
    }

    /// Whether this request belongs exclusively to the retained image language.
    pub const fn is_retained(&self) -> bool {
        self.retained
    }

    /// Decode only retained Number10 or Short Text11, without legacy fallback.
    pub fn decode_retained(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        let language = if bytes.starts_with(b"TCAC0010") {
            codec::Language::RetainedNumber
        } else if bytes.starts_with(b"TCAC0011") {
            codec::Language::RetainedShortText
        } else {
            return Err(CurrentFieldAdditionIntentError::InvalidRecord);
        };
        codec::decode(bytes, language, budget, cancelled)
    }

    /// Exact source-resolved table spelling, independent of page addresses.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Exact semantic declaration captured from the native candidate.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.target
    }

    /// Encode the original request and complete BEFORE binding, never an AFTER.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, CurrentFieldAdditionIntentError> {
        codec::encode(self, budget, cancelled)
    }

    /// Decode only TCAC0008/profile8 and its fixed Number declaration before allocation.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        codec::decode(bytes, codec::Language::Number, budget, cancelled)
    }

    /// Decode only TCAC0009/profile9 and its nullable Short Text declaration.
    /// The Number decoder remains closed to this language.
    pub fn decode_short_text(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CurrentFieldAdditionIntentError> {
        codec::decode(bytes, codec::Language::ShortText, budget, cancelled)
    }

    /// Verify exact complete original bytes before any native reconstruction.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CurrentFieldAdditionIntentError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.before_length {
            return Err(CurrentFieldAdditionIntentError::BeforeMismatch);
        }
        let actual = if self.retained {
            binding::digest_retained(bytes, budget, cancelled)?
        } else {
            binding::digest(bytes, budget, cancelled)?
        };
        if actual != self.before_binding {
            return Err(CurrentFieldAdditionIntentError::BeforeMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }
}

pub(super) fn validate_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurrentFieldAdditionIntentError> {
    check(budget, cancelled)?;
    if text.is_empty() || text.len() > MAX_NAME_BYTES {
        return Err(CurrentFieldAdditionIntentError::InvalidRecord);
    }
    budget.charge(Resource::Steps, text.len() as u64)?;
    if text.encode_utf16().count() > MAX_NAME_UNITS || text.chars().any(char::is_control) {
        return Err(CurrentFieldAdditionIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn validate_column(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CurrentFieldAdditionIntentError> {
    validate_name(text, budget, cancelled)?;
    if !text.is_ascii() {
        return Err(CurrentFieldAdditionIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn copy_name(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<SchemaName, CurrentFieldAdditionIntentError> {
    validate_name(text, budget, cancelled)?;
    budget.charge_allocation(text.len() as u64, 1)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(text);
    SchemaName::new(owned).map_err(|_| CurrentFieldAdditionIntentError::InvalidRecord)
}
