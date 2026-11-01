use super::{carrier, codec};
use crate::transaction::intent::{NativeIntentError, binding, model};
use crate::{FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::SchemaName;

/// One original column's owned requests, including empty groups and no-ops.
/// Construction does not resolve native metadata or grant write permission.
pub struct NativeFixedIntentGroup {
    column: SchemaName,
    updates: Vec<FixedScalarUpdate>,
}

impl NativeFixedIntentGroup {
    /// Move requests unchanged and copy the column spelling under the budget.
    pub fn new(
        column: &str,
        updates: Vec<FixedScalarUpdate>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        model::validate_name(column, budget, cancelled)?;
        if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }
        for update in &updates {
            check(budget, cancelled)?;
            if update.row() >= crate::limits::MAX_LIVE_ROWS {
                return Err(InspectError::StructureLimit.into());
            }
            budget
                .charge(Resource::Steps, (carrier::WIDTH * 2) as u64)
                .map_err(InspectError::from)?;
            carrier::encode(update.expected())?;
            carrier::encode(update.value())?;
        }
        let column = copy_name(column, budget)?;
        check(budget, cancelled)?;
        Ok(Self { column, updates })
    }

    /// Original spelling, resolved again against the source on every replay.
    pub fn column(&self) -> &str {
        self.column.as_str()
    }

    /// Exact supplied order and carriers, never macro-expanded final cells.
    pub fn updates(&self) -> &[FixedScalarUpdate] {
        &self.updates
    }
}

/// Ordered original fixed requests bound to one complete BEFORE image.
/// No program selectors, final rows, permission flags or candidate bytes occur here.
pub struct NativeFixedEditIntent {
    table: SchemaName,
    groups: Vec<NativeFixedIntentGroup>,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; 32],
}

impl NativeFixedEditIntent {
    /// Maximum encoded data record for this explicitly bounded request profile.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Bind moved original requests to exact bytes; this does not qualify a file.
    pub fn new(
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
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
        groups: Vec<NativeFixedIntentGroup>,
        before_length: u64,
        before_binding: [u8; 32],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        binding::length(before_length)?;
        model::validate_name(table, budget, cancelled)?;
        assignment_count(&groups, budget, cancelled)?;
        let table = copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            groups,
            before_length,
            before_binding,
        })
    }

    /// Original source table spelling, without a selector or file path.
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    /// Exact original column groups and assignment order.
    pub fn groups(&self) -> &[NativeFixedIntentGroup] {
        &self.groups
    }

    /// Exact original image length, not a native validity certificate.
    pub const fn before_length(&self) -> u64 {
        self.before_length
    }

    /// Borrow actual original groups for fresh native planning under paid storage.
    pub fn borrowed_groups(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<FixedColumnUpdates<'_>>, NativeIntentError> {
        borrow_groups(self.table(), &self.groups, budget, cancelled)
    }

    /// Verify complete source bytes without granting replay authority.
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

pub(in crate::transaction::intent) fn borrow_groups<'a>(
    table: &'a str,
    groups: &'a [NativeFixedIntentGroup],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FixedColumnUpdates<'a>>, NativeIntentError> {
    assignment_count(groups, budget, cancelled)?;
    budget
        .charge_allocation(
            groups.len() as u64,
            size_of::<FixedColumnUpdates<'_>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve_exact(groups.len())
        .map_err(|_| InspectError::Allocation)?;
    for group in groups {
        check(budget, cancelled)?;
        borrowed.push(FixedColumnUpdates::new(
            table,
            group.column(),
            group.updates(),
        ));
    }
    Ok(borrowed)
}

pub(super) fn assignment_count(
    groups: &[NativeFixedIntentGroup],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    if groups.is_empty() || groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    let mut count = 0_usize;
    for group in groups {
        check(budget, cancelled)?;
        count = count
            .checked_add(group.updates().len())
            .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
            .ok_or(InspectError::StructureLimit)?;
    }
    Ok(count)
}

fn copy_name(name: &str, budget: &mut ResourceBudget) -> Result<SchemaName, NativeIntentError> {
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, (name.len() as u64) * 2)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    SchemaName::new(owned)
        .map_err(InspectError::from)
        .map_err(Into::into)
}
