//! Owned immutable original fitted requests, without native admission authority.
use super::{carrier, codec};
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate,
    header::check,
    transaction::intent::{NativeIntentError, binding, model as names},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::SchemaName;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Fixed,
    Variable,
}

/// Exact original request category, retained even for an empty column group.
/// Payload Debug and Display are deliberately unavailable.
pub enum NativeFittedUpdates {
    /// Fixed scalar requests with exact original expected/replacement carriers.
    Fixed(Vec<FixedScalarUpdate>),
    /// Short Text/Binary requests, retaining NULL and present empty separately.
    Variable(Vec<SlotScalarUpdate>),
}

impl NativeFittedUpdates {
    pub(super) fn family(&self) -> Family {
        match self {
            Self::Fixed(_) => Family::Fixed,
            Self::Variable(_) => Family::Variable,
        }
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Fixed(updates) => updates.len(),
            Self::Variable(updates) => updates.len(),
        }
    }
}

/// An owned column spelling and moved original requests, never final cells.
pub struct NativeFittedIntentGroup {
    column: SchemaName,
    updates: NativeFittedUpdates,
}

impl NativeFittedIntentGroup {
    /// Move row-unique original requests and copy the exact column spelling.
    pub fn new(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        names::validate_name(column, budget, cancelled)?;
        if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }
        match &updates {
            NativeFittedUpdates::Fixed(updates) => {
                FixedScalarUpdate::validate_intent_requests(updates, budget, cancelled)?;
                for update in updates {
                    check(budget, cancelled)?;
                    carrier::validate(update.expected(), Family::Fixed, budget, cancelled)?;
                    carrier::validate(update.value(), Family::Fixed, budget, cancelled)?;
                }
            }
            NativeFittedUpdates::Variable(updates) => {
                SlotScalarUpdate::validate_intent_requests(updates, budget, cancelled)?;
                for update in updates {
                    check(budget, cancelled)?;
                    carrier::validate(update.expected(), Family::Variable, budget, cancelled)?;
                    carrier::validate(update.value(), Family::Variable, budget, cancelled)?;
                }
            }
        }
        codec::group_parts_length(column, &updates, budget, cancelled)?;
        let column = names::copy_name(column, budget)?;
        check(budget, cancelled)?;
        Ok(Self { column, updates })
    }

    /// Original column spelling, resolved afresh by every native planner.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Original family and written request order, including empty groups.
    pub fn updates(&self) -> &NativeFittedUpdates {
        &self.updates
    }
}

/// Exact ordered original groups bound to a complete BEFORE byte image.
/// Construction/decoding does not qualify a native table or authorize recovery.
pub struct NativeFittedEditIntent {
    table: SchemaName,
    groups: Vec<NativeFittedIntentGroup>,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; 32],
}

impl NativeFittedEditIntent {
    /// Current complete-record ceiling, independent of native file eligibility.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Serialized variable-carrier ceiling; native field capacity is separate.
    pub const MAX_VARIABLE_BYTES: usize = carrier::MAX_VARIABLE_BYTES;

    /// Bind moved original groups to the exact complete BEFORE bytes.
    pub fn new(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let mut result = Self::from_decoded(
            table,
            groups,
            before.len() as u64,
            [0; 32],
            budget,
            cancelled,
        )?;
        result.before_binding = binding::digest(before, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(result)
    }

    pub(super) fn from_decoded(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before_length: u64,
        before_binding: [u8; 32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        binding::length(before_length)?;
        names::validate_name(table, budget, cancelled)?;
        validate_groups(&groups, budget, cancelled)?;
        // Check the complete eventual wire before the constructor's name copy.
        // Existing moved request allocations remain caller-owned inputs.
        codec::parts_length(table, &groups, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        let result = Self {
            table,
            groups,
            before_length,
            before_binding,
        };
        check(budget, cancelled)?;
        Ok(result)
    }

    /// Exact original source table spelling; no file or namespace selector.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Original ordered column groups, without macro-expanded final values.
    pub fn groups(&self) -> &[NativeFittedIntentGroup] {
        &self.groups
    }

    /// Exact source byte length, which alone grants no native authority.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Borrow the existing native fitted request types without cloning payloads.
    pub fn borrowed_groups(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<FittedColumnUpdates<'_>>, NativeIntentError> {
        validate_groups(&self.groups, budget, cancelled)?;
        budget
            .charge_allocation(
                self.groups.len() as u64,
                size_of::<FittedColumnUpdates<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(self.groups.len())
            .map_err(|_| InspectError::Allocation)?;
        for group in &self.groups {
            check(budget, cancelled)?;
            result.push(match group.updates() {
                NativeFittedUpdates::Fixed(updates) => FittedColumnUpdates::Fixed {
                    column: group.column(),
                    updates,
                },
                NativeFittedUpdates::Variable(updates) => FittedColumnUpdates::Variable {
                    column: group.column(),
                    updates,
                },
            });
        }
        check(budget, cancelled)?;
        Ok(result)
    }

    /// Check complete original-image binding without authorizing replay.
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
        let actual = binding::digest(before, budget, cancelled)?;
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

pub(super) fn validate_groups(
    groups: &[NativeFittedIntentGroup],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    check(budget, cancelled)?;
    if groups.is_empty() || groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut count = 0_usize;
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.updates.len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
        for prior in &groups[..position] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    prior.column().len().max(group.column().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if prior.column().eq_ignore_ascii_case(group.column()) {
                return Err(EditError::Conflict.into());
            }
        }
    }
    check(budget, cancelled)?;
    Ok(count)
}

pub(super) struct Cell<'a> {
    pub row: usize,
    pub expected: &'a Value,
    pub value: &'a Value,
}

pub(super) enum Cells<'a> {
    Fixed(std::slice::Iter<'a, FixedScalarUpdate>),
    Variable(std::slice::Iter<'a, SlotScalarUpdate>),
}

impl<'a> Iterator for Cells<'a> {
    type Item = Cell<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Fixed(updates) => updates.next().map(|update| Cell {
                row: update.row(),
                expected: update.expected(),
                value: update.value(),
            }),
            Self::Variable(updates) => updates.next().map(|update| Cell {
                row: update.row(),
                expected: update.expected(),
                value: update.value(),
            }),
        }
    }
}

impl NativeFittedUpdates {
    pub(super) fn cells(&self) -> Cells<'_> {
        match self {
            Self::Fixed(updates) => Cells::Fixed(updates.iter()),
            Self::Variable(updates) => Cells::Variable(updates.iter()),
        }
    }
}
