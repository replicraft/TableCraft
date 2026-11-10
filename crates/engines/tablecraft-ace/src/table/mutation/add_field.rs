//! Current-schema nullable declarations without rewriting existing records.
mod catalog;
mod definition;
mod properties;
mod verify;

use crate::{
    EditError, InspectError, NativeImage,
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::{catalog_property, current_schema},
};
use catalog::DescriptorPatch;
use definition::Plan;
use tablecraft_core::{
    ids::ObjectId,
    limits::{LimitError, Resource, ResourceBudget},
};
use thiserror::Error;

#[derive(Clone, Copy)]
enum Target {
    Number,
    ShortText {
        max_utf16_units: usize,
        unicode_compression: bool,
    },
}

impl Target {
    fn property_keys(self) -> [&'static str; 2] {
        match self {
            Self::Number => ["DefaultValue", "Required"],
            Self::ShortText { .. } => ["Required", "AllowZeroLength"],
        }
    }
}

/// Payload-free failure of current native Number column addition.
#[derive(Debug, Error)]
pub enum CurrentFieldAdditionError {
    /// Native parsing, I/O, cancellation, allocation or cumulative limits failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Current programs, records, constraints or owned placement were refused.
    #[error(transparent)]
    Native(#[from] EditError),
    /// Current native roles, declarations or dependencies are unsupported.
    #[error("native table is outside the current field-addition source profile")]
    Source,
    /// The proposed name is not a valid qualified ASCII identifier.
    #[error("current Number addition requires a valid ASCII field name")]
    InvalidName,
    /// An existing native field has the proposed name under ASCII folding.
    #[error("field addition conflicts with an existing field")]
    DuplicateName,
    /// The descriptor, name or complete new property body cannot fit owned space.
    #[error("current field addition exceeds existing owned capacity")]
    Capacity,
    /// The candidate changed an unrelated byte or decoded native fact.
    #[error("current field-addition preservation check failed")]
    Preservation,
}

impl From<LimitError> for CurrentFieldAdditionError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

impl From<current_schema::Error> for CurrentFieldAdditionError {
    fn from(error: current_schema::Error) -> Self {
        match error {
            current_schema::Error::Inspect(error) => Self::Inspect(error),
            current_schema::Error::Native(error) => Self::Native(error),
            current_schema::Error::Source => Self::Source,
        }
    }
}

/// Original native identity and measured fitting capacity, not save authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentFieldAdditionSource {
    object: ObjectId,
    number: usize,
    column_id: u16,
    maximum_name_units: usize,
}

impl CurrentFieldAdditionSource {
    /// Actual table identity in the caller's catalog scope.
    pub const fn object_id(self) -> ObjectId {
        self.object
    }

    /// Next qualified native field number, independent of a display caption.
    pub const fn native_number(self) -> usize {
        self.number
    }

    /// New stored creation ID after complete existing-ID validation.
    pub const fn column_id(self) -> u16 {
        self.column_id
    }

    /// Largest ASCII name fitting current owned metadata; zero means none fits.
    pub const fn maximum_name_utf16_units(self) -> usize {
        self.maximum_name_units
    }
}

/// Sealed nullable LONG / Required(false) / literal-zero-default metadata change.
pub struct NativeCurrentFieldAdditionCandidate {
    proof: current_schema::Source,
    after: Vec<u8>,
    column: String,
    facts: CurrentFieldAdditionSource,
}

impl NativeCurrentFieldAdditionCandidate {
    /// Complete immutable original, including historical record and free bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.proof.image().as_bytes()
    }

    /// Complete metadata-only replacement; existing records remain unchanged.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.proof.table().schema.name()
    }

    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Native number allocated from the validated declaration high-water mark.
    pub const fn native_number(&self) -> usize {
        self.facts.number
    }

    /// New stored creation-time field ID.
    pub const fn column_id(&self) -> u16 {
        self.facts.column_id
    }

    /// Number of declared live fields in the replacement.
    pub const fn column_count(&self) -> usize {
        self.facts.number + 1
    }
}

/// Sealed nullable Short Text / Required(false) / AllowZeroLength(true) metadata change.
/// No DefaultValue is stored; historical absent values remain NULL.
pub struct NativeCurrentShortTextFieldAdditionCandidate {
    proof: current_schema::Source,
    after: Vec<u8>,
    column: String,
    facts: CurrentFieldAdditionSource,
    max_utf16_units: usize,
    unicode_compression: bool,
}

impl NativeCurrentShortTextFieldAdditionCandidate {
    /// Explicit native capacity in UTF-16 code units.
    pub const fn max_utf16_units(&self) -> usize {
        self.max_utf16_units
    }

    /// Explicit Unicode-compression permission.
    pub const fn unicode_compression(&self) -> bool {
        self.unicode_compression
    }

    /// Complete immutable original, including historical record and free bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.proof.image().as_bytes()
    }

    /// Complete metadata-only replacement; existing records remain unchanged.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.proof.table().schema.name()
    }

    /// Exact requested new field spelling.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Native number allocated from the validated declaration high-water mark.
    pub const fn native_number(&self) -> usize {
        self.facts.number
    }

    /// New stored creation-time field ID.
    pub const fn column_id(&self) -> u16 {
        self.facts.column_id
    }

    /// Number of declared live fields in the replacement.
    pub const fn column_count(&self) -> usize {
        self.facts.number + 1
    }
}

impl NativeImage {
    /// Qualify current table ownership and exact Number-name fitting capacity.
    /// No replacement name or candidate image is invented for this inspection.
    pub fn inspect_current_field_addition_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CurrentFieldAdditionSource, CurrentFieldAdditionError> {
        let current = current_schema::observe(self, table, budget, cancelled)?;
        let plan = Plan::capture(self, &current.schema, budget, cancelled)?;
        facts(self, &current, &plan, Target::Number, budget, cancelled)
    }

    /// Add nullable LONG with Required(false) and a stored zero default.
    /// Existing rows are not expanded or backfilled; the new field reads NULL
    /// in historical records, while future omitted insertion uses its default.
    pub fn prepare_current_number_field_addition(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentFieldAdditionCandidate, CurrentFieldAdditionError> {
        let _span = tracing::debug_span!("native.field_addition.prepare_current_number").entered();
        let parts = prepare(self, table, column, Target::Number, budget, cancelled)?;
        Ok(NativeCurrentFieldAdditionCandidate {
            proof: parts.proof,
            after: parts.after,
            column: parts.column,
            facts: parts.facts,
        })
    }
    /// Inspect exact current Short Text name capacity without inventing a candidate.
    pub fn inspect_current_short_text_field_addition_source(
        &self,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CurrentFieldAdditionSource, CurrentFieldAdditionError> {
        let current = current_schema::observe(self, table, budget, cancelled)?;
        let plan = Plan::capture(self, &current.schema, budget, cancelled)?;
        facts(
            self,
            &current,
            &plan,
            Target::ShortText {
                max_utf16_units: 255,
                unicode_compression: true,
            },
            budget,
            cancelled,
        )
    }

    /// Append nullable Short Text without modifying or backfilling any old record.
    /// Required(false), AllowZeroLength(true), and absent DefaultValue remain distinct.
    pub fn prepare_current_short_text_field_addition(
        self,
        table: &str,
        column: &str,
        max_utf16_units: usize,
        unicode_compression: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeCurrentShortTextFieldAdditionCandidate, CurrentFieldAdditionError> {
        let _span =
            tracing::debug_span!("native.field_addition.prepare_current_short_text").entered();
        if !(1..=crate::format::data_type::MAX_SHORT_TEXT_UNITS).contains(&max_utf16_units) {
            return Err(CurrentFieldAdditionError::Source);
        }
        let target = Target::ShortText {
            max_utf16_units,
            unicode_compression,
        };
        let parts = prepare(self, table, column, target, budget, cancelled)?;
        Ok(NativeCurrentShortTextFieldAdditionCandidate {
            proof: parts.proof,
            after: parts.after,
            column: parts.column,
            facts: parts.facts,
            max_utf16_units,
            unicode_compression,
        })
    }
}

struct CandidateParts {
    proof: current_schema::Source,
    after: Vec<u8>,
    column: String,
    facts: CurrentFieldAdditionSource,
}

fn prepare(
    image: NativeImage,
    table: &str,
    column: &str,
    target: Target,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CandidateParts, CurrentFieldAdditionError> {
    check(budget, cancelled)?;
    if !crate::identifier::valid_name(column, budget, cancelled)? || !column.is_ascii() {
        return Err(CurrentFieldAdditionError::InvalidName);
    }
    let proof = current_schema::Source::capture(image, table, budget, cancelled)?;
    let current = proof.table();
    let plan = Plan::capture(proof.image(), &current.schema, budget, cancelled)?;
    let facts = facts(proof.image(), current, &plan, target, budget, cancelled)?;
    if column.len() > facts.maximum_name_units {
        return Err(CurrentFieldAdditionError::Capacity);
    }
    for field in current.schema.columns() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, field.name().len().max(column.len()) as u64)?;
        if field.name().eq_ignore_ascii_case(column) {
            return Err(CurrentFieldAdditionError::DuplicateName);
        }
    }
    let body = properties::append(&current.properties, column, target, budget, cancelled)?;

    let append = catalog_property::append_fitted_body(proof.image(), &body, budget, cancelled)?;
    if append.length != proof.image().as_bytes().len() || !append.maps.is_empty() {
        return Err(CurrentFieldAdditionError::Preservation);
    }
    let descriptor = DescriptorPatch::capture(
        proof.image(),
        &current.catalog,
        current.schema.object_id(),
        append.reference,
        body.len(),
        budget,
        cancelled,
    )?;
    let mut after = plan.build(proof.image(), column, target, budget, cancelled)?;
    let page = append.page as usize * PAGE_BYTES;
    after
        .get_mut(page..page + PAGE_BYTES)
        .ok_or(CurrentFieldAdditionError::Preservation)?
        .copy_from_slice(&append.bytes);
    descriptor.apply(&mut after)?;

    verify::candidate(
        &proof,
        &plan,
        target,
        &descriptor,
        &after,
        column,
        &body,
        append.reference,
        budget,
        cancelled,
    )?;

    budget.charge_allocation(column.len() as u64, 1)?;
    let mut name = String::new();
    name.try_reserve_exact(column.len())
        .map_err(|_| InspectError::Allocation)?;
    name.push_str(column);

    Ok(CandidateParts {
        proof,
        after,
        column: name,
        facts,
    })
}

fn facts(
    image: &NativeImage,
    current: &current_schema::Table,
    plan: &Plan,
    target: Target,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CurrentFieldAdditionSource, CurrentFieldAdditionError> {
    check(budget, cancelled)?;
    if matches!(target, Target::ShortText { .. }) {
        let order = crate::inspect_database_sort_order(&mut image.reader(), budget, cancelled)?;
        if order.general_family() != Some(crate::GeneralSortFamily::General2010) {
            return Err(CurrentFieldAdditionError::Source);
        }
    }
    budget.charge(Resource::Steps, current.schema.name().len() as u64)?;
    if !current.schema.name().is_ascii() {
        return Err(CurrentFieldAdditionError::Source);
    }

    for field in current.schema.columns() {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, field.name().len() as u64)?;
        if !field.name().is_ascii() {
            return Err(CurrentFieldAdditionError::Source);
        }
    }
    let capacity =
        catalog_property::fitted_property_capacity(image, budget, cancelled)?.unwrap_or(0);
    let base = properties::body_bytes(&current.properties, 0, target, budget, cancelled)?;
    let mut maximum_name_units = capacity
        .checked_sub(base)
        .map_or(0, |bytes| bytes / size_of::<u16>())
        .min(plan.maximum_name_units);
    if !DescriptorPatch::check_capacity(
        image,
        &current.catalog,
        current.schema.object_id(),
        budget,
        cancelled,
    )? {
        maximum_name_units = 0;
    }
    Ok(CurrentFieldAdditionSource {
        object: current.schema.object_id(),
        number: plan.number,
        column_id: plan.column_id,
        maximum_name_units,
    })
}

#[cfg(test)]
mod tests;
