//! Owned immutable original fitted requests, without native admission authority.
use super::{carrier, codec, grammar::Grammar};
use crate::{
    EditError, FittedColumnUpdates, FixedScalarUpdate, InspectError, SlotScalarUpdate,
    header::check,
    transaction::intent::{NativeIntentError, model as names},
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
        Self::new_with_grammar(column, updates, Grammar::Legacy, budget, cancelled)
    }

    /// Retain original requests under TCNI17's actual data representation widths.
    /// Native field fit and operation eligibility remain consuming checks.
    pub fn new_retained(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(column, updates, Grammar::Retained, budget, cancelled)
    }

    /// Original TCNI26 indexed LONG group, including zero-match requests.
    pub fn new_retained_indexed_long(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            column,
            updates,
            Grammar::RetainedIndexedLong,
            budget,
            cancelled,
        )
    }

    /// Original TCNI25 fixed LONG group, retaining every requested row.
    pub fn new_retained_long_column_batch(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            column,
            updates,
            Grammar::RetainedLongColumnBatch,
            budget,
            cancelled,
        )
    }

    /// Original TCNI27 exact Currency group, retaining every requested row.
    ///
    /// # Errors
    /// Rejects invalid names, singleton groups, non-Currency carriers, duplicate
    /// identities, allocation failure, cancellation and exhausted caller work.
    pub fn new_retained_currency_column_batch(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            column,
            updates,
            Grammar::RetainedCurrencyColumnBatch,
            budget,
            cancelled,
        )
    }

    /// Retain only TCNI21 original exact fixed-scalar carriers.
    pub fn new_retained_fixed_scalars(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        if !matches!(updates, NativeFittedUpdates::Fixed(_)) {
            return Err(NativeIntentError::InvalidRecord);
        }
        Self::new_with_grammar(
            column,
            updates,
            Grammar::RetainedFixedScalars,
            budget,
            cancelled,
        )
    }

    /// Retain TCNI22 GUID fixed groups or exact Decimal variable groups.
    pub fn new_retained_wide_values(
        column: &str,
        updates: NativeFittedUpdates,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            column,
            updates,
            Grammar::RetainedWideValues,
            budget,
            cancelled,
        )
    }

    pub(super) fn new_with_grammar(
        column: &str,
        updates: NativeFittedUpdates,
        grammar: Grammar,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        if matches!(
            grammar,
            Grammar::RetainedFixedScalars
                | Grammar::RetainedEmpty
                | Grammar::RetainedLongColumnBatch
                | Grammar::RetainedCurrencyColumnBatch
                | Grammar::RetainedIndexedLong
        ) && !matches!(updates, NativeFittedUpdates::Fixed(_))
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        if grammar == Grammar::RetainedLongColumnBatch && updates.len() < 2 {
            return Err(NativeIntentError::InvalidRecord);
        }
        if grammar == Grammar::RetainedCurrencyColumnBatch && updates.len() == 1 {
            return Err(NativeIntentError::InvalidRecord);
        }
        grammar.name(column, budget, cancelled)?;
        if updates.len() > grammar.assignments() {
            return Err(InspectError::StructureLimit.into());
        }
        match &updates {
            NativeFittedUpdates::Fixed(updates) => {
                if grammar == Grammar::Legacy {
                    FixedScalarUpdate::validate_intent_requests(updates, budget, cancelled)?;
                } else {
                    validate_rows(
                        updates.iter().map(FixedScalarUpdate::row),
                        budget,
                        cancelled,
                    )?;
                }
                for update in updates {
                    check(budget, cancelled)?;
                    validate_value(update.expected(), Family::Fixed, grammar, budget, cancelled)?;
                    validate_value(update.value(), Family::Fixed, grammar, budget, cancelled)?;
                }
            }
            NativeFittedUpdates::Variable(updates) => {
                if grammar == Grammar::Legacy {
                    SlotScalarUpdate::validate_intent_requests(updates, budget, cancelled)?;
                } else {
                    validate_rows(updates.iter().map(SlotScalarUpdate::row), budget, cancelled)?;
                }
                for update in updates {
                    check(budget, cancelled)?;
                    validate_value(
                        update.expected(),
                        Family::Variable,
                        grammar,
                        budget,
                        cancelled,
                    )?;
                    validate_value(update.value(), Family::Variable, grammar, budget, cancelled)?;
                }
            }
        }
        codec::group_parts_length(column, &updates, grammar, budget, cancelled)?;
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
    pub(super) grammar: Grammar,
    pub(super) before_length: u64,
    pub(super) before_binding: [u8; 32],
}

impl NativeFittedEditIntent {
    /// Current complete-record ceiling, independent of native file eligibility.
    pub const MAX_ENCODED_BYTES: usize = codec::MAX_RECORD_BYTES;

    /// Actual TCNI17 complete-record u32 length representation.
    pub const MAX_RETAINED_ENCODED_BYTES: usize = u32::MAX as usize;

    /// Actual TCNI17 variable-carrier u32 length representation.
    pub const MAX_RETAINED_VARIABLE_BYTES: usize = u32::MAX as usize;

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
        Self::new_with_grammar(table, groups, before, Grammar::Legacy, budget, cancelled)
    }

    /// Bind TCNI17 original requests to the complete retained source image.
    pub fn new_retained(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(table, groups, before, Grammar::Retained, budget, cancelled)
    }

    /// Bind only named fixed groups with no row assignments to the complete image.
    /// This inert request does not qualify target columns or native source authority.
    ///
    /// # Errors
    /// Refuses empty/duplicate target lists, variable or nonempty groups, invalid
    /// names/image geometry, allocation failure, cancellation and exhausted work.
    pub fn new_retained_empty(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedEmpty,
            budget,
            cancelled,
        )
    }

    /// Bind TCNI26 indexed LONG originals to the complete immutable source.
    pub fn new_retained_indexed_long(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedIndexedLong,
            budget,
            cancelled,
        )
    }

    /// Bind the distinct TCNI25 one-column LONG batch to all BEFORE bytes.
    pub fn new_retained_long_column_batch(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedLongColumnBatch,
            budget,
            cancelled,
        )
    }

    /// Bind the distinct TCNI27 one-column Currency batch to all BEFORE bytes.
    ///
    /// # Errors
    /// Rejects invalid group/name/carrier framing, source-length representation,
    /// allocation failure, cancellation or exhausted caller work. Native source
    /// eligibility remains a separate consuming qualification.
    pub fn new_retained_currency_column_batch(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedCurrencyColumnBatch,
            budget,
            cancelled,
        )
    }

    /// Bind the closed TCNI21 scalar language to a complete native-sized image.
    pub fn new_retained_fixed_scalars(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedFixedScalars,
            budget,
            cancelled,
        )
    }

    /// Bind TCNI22 originals to a complete native-sized BEFORE image.
    pub fn new_retained_wide_values(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        Self::new_with_grammar(
            table,
            groups,
            before,
            Grammar::RetainedWideValues,
            budget,
            cancelled,
        )
    }

    fn new_with_grammar(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before: &[u8],
        grammar: Grammar,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        let mut result = Self::from_decoded_with_grammar(
            table,
            groups,
            before.len() as u64,
            [0; 32],
            grammar,
            budget,
            cancelled,
        )?;
        result.before_binding = grammar.digest(before, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(result)
    }

    pub(super) fn from_decoded_with_grammar(
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        before_length: u64,
        before_binding: [u8; 32],
        grammar: Grammar,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeIntentError> {
        check(budget, cancelled)?;
        grammar.length(before_length)?;
        grammar.name(table, budget, cancelled)?;
        validate_groups(&groups, grammar, budget, cancelled)?;
        codec::parts_length(table, &groups, grammar, budget, cancelled)?;
        let table = names::copy_name(table, budget)?;
        check(budget, cancelled)?;
        Ok(Self {
            table,
            groups,
            grammar,
            before_length,
            before_binding,
        })
    }

    pub(in crate::transaction) fn require_retained_currency_column_batch(
        &self,
    ) -> Result<(), NativeIntentError> {
        if self.grammar != Grammar::RetainedCurrencyColumnBatch {
            return Err(NativeIntentError::UnsupportedProfile);
        }
        Ok(())
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
        validate_groups(&self.groups, self.grammar, budget, cancelled)?;
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
        let actual = self.grammar.digest(before, budget, cancelled)?;
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
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeIntentError> {
    check(budget, cancelled)?;
    if groups.is_empty() || groups.len() > grammar.groups() {
        return Err(InspectError::StructureLimit.into());
    }
    if matches!(
        grammar,
        Grammar::RetainedLongColumnBatch
            | Grammar::RetainedCurrencyColumnBatch
            | Grammar::RetainedIndexedLong
    ) {
        let [group] = groups else {
            return Err(NativeIntentError::InvalidRecord);
        };
        let NativeFittedUpdates::Fixed(updates) = group.updates() else {
            return Err(NativeIntentError::InvalidRecord);
        };
        if grammar == Grammar::RetainedLongColumnBatch && updates.len() < 2 {
            return Err(NativeIntentError::InvalidRecord);
        }
        if grammar == Grammar::RetainedCurrencyColumnBatch && updates.len() == 1 {
            return Err(NativeIntentError::InvalidRecord);
        }
        for update in updates {
            check(budget, cancelled)?;
            grammar.view(update.expected(), Family::Fixed)?;
            grammar.view(update.value(), Family::Fixed)?;
        }
    }
    let mut count = 0_usize;
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        if matches!(
            grammar,
            Grammar::RetainedFixedScalars
                | Grammar::RetainedEmpty
                | Grammar::RetainedLongColumnBatch
                | Grammar::RetainedCurrencyColumnBatch
                | Grammar::RetainedIndexedLong
        ) && !matches!(group.updates(), NativeFittedUpdates::Fixed(_))
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        if grammar != Grammar::Legacy {
            grammar.name(group.column(), budget, cancelled)?;
        } else if group.column().len() > names::MAX_NAME_BYTES {
            // A group constructed for TCNI17 cannot bypass a legacy name bound.
            return Err(NativeIntentError::InvalidRecord);
        }
        count = count
            .checked_add(group.updates.len())
            .filter(|count| *count <= grammar.assignments())
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

fn validate_rows(
    rows: impl Iterator<Item = usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    let mut seen = crate::transaction::intent::row_ordinals::RowOrdinals::default();
    for row in rows {
        seen.insert(row, budget, cancelled)?;
    }
    Ok(())
}

fn validate_value(
    value: &Value,
    family: Family,
    grammar: Grammar,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeIntentError> {
    if grammar == Grammar::Legacy {
        return carrier::validate(value, family, budget, cancelled);
    }
    let encoded = grammar.view(value, family)?;
    carrier::pay_scan(encoded.bytes(), budget, cancelled)
}
