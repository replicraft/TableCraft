//! Unequal-width ASCII names fit existing owned pages and retain every row slot.
mod build;
mod layout;
mod populated;
pub use populated::NativePopulatedFieldRenameCandidate;
#[cfg(test)]
mod tests;
mod verify;

use super::{FieldRenameError as Error, NativeFieldRenameIdentity, source::Qualified};
use crate::{NativeImage, header::check, table::mutation::saved_field::Observed};
use layout::Layout;
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Complete original source/packing facts, not permission to rename or save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FittedFieldRenameSource {
    identity: NativeFieldRenameIdentity,
    maximum_name_units: usize,
}

impl FittedFieldRenameSource {
    /// Original native field number, stored ID and exact original name width.
    pub const fn identity(self) -> NativeFieldRenameIdentity {
        self.identity
    }

    /// Largest ASCII spelling that can fit both existing physical owners.
    /// The empty fitted operation excludes the original width; its same-width
    /// requests use the original candidate. The populated operation admits all
    /// widths through its separate source-qualified candidate.
    pub const fn maximum_name_utf16_units(self) -> usize {
        self.maximum_name_units
    }
}

struct Prepared {
    source: Qualified,
    after: Vec<u8>,
    new_name: String,
}

/// Sealed unequal-width rename over one complete settled empty source.
/// This supplies no journal, repeated-rename, ADD or general schema authority.
pub struct NativeFittedFieldRenameCandidate {
    prepared: Prepared,
}

impl NativeFittedFieldRenameCandidate {
    /// Complete source-qualified original including historical property rows.
    pub fn before_bytes(&self) -> &[u8] {
        self.prepared.source.image().as_bytes()
    }

    /// Complete same-length candidate with parser-bound relocated name frames.
    pub fn after_bytes(&self) -> &[u8] {
        &self.prepared.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.prepared.source.observed.schema.name()
    }

    /// Actual original field spelling.
    pub fn column(&self) -> &str {
        self.prepared.source.observed.column().name()
    }

    /// Exact requested unequal-width ASCII spelling.
    pub fn new_name(&self) -> &str {
        &self.prepared.new_name
    }

    /// Original native field identity, distinct from a visible position.
    pub const fn identity(&self) -> NativeFieldRenameIdentity {
        NativeFieldRenameIdentity::from_observed(self.prepared.source.observed.identity)
    }

    /// This separate candidate always changes width and therefore spelling.
    pub const fn changed(&self) -> bool {
        true
    }
}

impl NativeImage {
    /// Prove original source and actual fitting geometry without inventing a name.
    pub fn inspect_fitted_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FittedFieldRenameSource, Error> {
        self.check_field_history_name_auto_correct_off_source(budget, cancelled)?;
        let observed =
            Observed::capture(self, table, column, budget, cancelled).map_err(Error::from)?;
        let layout = Layout::capture(self, &observed, budget, cancelled)?;
        Ok(FittedFieldRenameSource {
            identity: NativeFieldRenameIdentity::from_observed(observed.identity),
            maximum_name_units: layout.maximum_name_units,
        })
    }

    /// Repack one unequal-width ASCII name within the already owned TDEF/LVAL
    /// capacity. Existing same-width and no-op languages remain unchanged.
    pub fn prepare_fitted_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFittedFieldRenameCandidate, Error> {
        let _span = tracing::debug_span!("native.field_rename.prepare_fitted").entered();
        check(budget, cancelled)?;
        if !crate::identifier::valid_name(new_name, budget, cancelled)? {
            return Err(Error::InvalidName);
        }
        let source = Qualified::capture(self, table, column, budget, cancelled)?;
        if new_name.len() == source.observed.column().name().len() {
            return Err(Error::FittedNameShape);
        }
        let prepared = prepare(source, new_name, budget, cancelled)?;
        Ok(NativeFittedFieldRenameCandidate { prepared })
    }
}

fn prepare(
    source: Qualified,
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, Error> {
    let old_name = source.observed.column().name();
    if !new_name.is_ascii() {
        return Err(Error::FittedNameShape);
    }
    let layout = Layout::qualified(&source, budget, cancelled)?;
    if new_name.len() > layout.maximum_name_units {
        return Err(Error::FittedCapacity);
    }
    for other in source.observed.schema.columns() {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            other.name().len().max(new_name.len()) as u64,
        )?;
        if other.number() != source.observed.identity.column_number()
            && other.name().eq_ignore_ascii_case(new_name)
        {
            return Err(Error::DuplicateName);
        }
    }

    let units = isize::try_from(new_name.len()).map_err(|_| Error::FittedNameShape)?
        - isize::try_from(old_name.len()).map_err(|_| Error::FittedNameShape)?;
    let delta = units * size_of::<u16>() as isize;
    let after = build::candidate(&source, &layout, new_name, delta, budget, cancelled)?;
    verify::candidate(&source, &layout, &after, new_name, delta, budget, cancelled)?;
    budget.charge_allocation(new_name.len() as u64, 1)?;
    let mut replacement = String::new();
    replacement
        .try_reserve_exact(new_name.len())
        .map_err(|_| crate::InspectError::Allocation)?;
    replacement.push_str(new_name);
    Ok(Prepared {
        source,
        after,
        new_name: replacement,
    })
}
