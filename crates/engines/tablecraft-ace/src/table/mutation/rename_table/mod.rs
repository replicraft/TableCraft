//! Source-qualified persisted catalog renaming, without dependency repair.
#[cfg(test)]
mod indexed_tests;
mod indexes;
mod page;
mod record;
mod source;
#[cfg(test)]
mod tests;

use crate::{InspectError, NativeImage, header::check};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::Value,
};
use thiserror::Error;

const MAX_NAME_UNITS: usize = 64;

/// Payload-free refusal of an unqualified native table rename.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TableRenameError {
    /// Native inspection, cancellation, allocation or resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// A native identifier rule or the 64-UTF-16-unit ceiling was violated.
    #[error("table rename requires a valid native identifier")]
    InvalidName,
    /// The requested spelling conflicts with another catalog name.
    #[error("table rename conflicts with an existing native object")]
    DuplicateName,
    /// Source geometry, catalog indexes or native storage are unqualified.
    #[error("native table rename source is unsupported")]
    Source,
    /// Name AutoCorrect or an application/schema dependency is unqualified.
    #[error("native table rename requires qualified disabled name tracking and no dependencies")]
    Dependency,
    /// The renamed catalog record cannot fit its existing DATA page.
    #[error("native table rename exceeds existing catalog page capacity")]
    Capacity,
    /// A reconstructed candidate differs outside its exact catalog rename scope.
    #[error("native table rename preservation proof failed")]
    Preservation,
}

/// Captured semantic and persistent native identity of the renamed table.
/// The scoped object identity is not a durable journal identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTableRenameIdentity {
    object_id: ObjectId,
    native_id: i32,
    parent_id: i32,
    definition_page: u32,
    catalog_page: u32,
    catalog_slot: u8,
}

impl NativeTableRenameIdentity {
    /// Caller-scoped object identity from the original catalog.
    pub const fn object_id(self) -> ObjectId {
        self.object_id
    }

    /// Original signed native MSysObjects Id.
    pub const fn native_id(self) -> i32 {
        self.native_id
    }

    /// Original native containment identity.
    pub const fn parent_id(self) -> i32 {
        self.parent_id
    }

    /// Original user-table definition page.
    pub const fn definition_page(self) -> u32 {
        self.definition_page
    }

    /// Original logical catalog DATA page.
    pub const fn catalog_page(self) -> u32 {
        self.catalog_page
    }

    /// Original logical catalog row slot, without row reordering.
    pub const fn catalog_slot(self) -> u8 {
        self.catalog_slot
    }
}

/// An owning, unpublished native table rename with exact BEFORE/AFTER images.
///
/// Both planners require explicit disabled Name AutoCorrect, one ordinary
/// local user table and a closed, dependency-free catalog. They do not repair
/// NameMaps, saved queries, forms, macros, relationships or catalog indexes.
/// The original table definition, data, native identities, timestamps and opaque
/// complement are retained. Native Access DateUpdate timing remains unqualified;
/// the planner conservatively preserves its original bytes. No Access execution
/// comparison is implied by independent byte-oracle qualification.
/// Persistence requires the separately checked schema journal operation, which
/// currently admits only the original unindexed planner profile.
pub struct NativeTableRenamePlan {
    before: NativeImage,
    after: Vec<u8>,
    table: String,
    new_name: String,
    identity: NativeTableRenameIdentity,
}

impl NativeTableRenamePlan {
    /// Actual original catalog spelling, independent of caller lookup spelling.
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Exact requested spelling, without normalization.
    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    /// Complete captured original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete unpublished candidate, with only the qualified catalog edit.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Original persistent and caller-scoped identities.
    pub const fn identity(&self) -> NativeTableRenameIdentity {
        self.identity
    }

    /// Whether the requested spelling changes the original spelling.
    pub fn changed(&self) -> bool {
        self.table != self.new_name
    }
}

impl NativeImage {
    /// Rename one qualified local table by changing its actual catalog Name.
    ///
    /// Exact spelling no-ops still perform complete source/dependency admission.
    /// Case-only names preserve the requested spelling. The existing catalog page
    /// must have sufficient free space; no page allocation or row relocation is
    /// attempted, and all non-Name record contents are preserved independently.
    pub fn plan_table_rename(
        self,
        table: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableRenamePlan, TableRenameError> {
        self.plan_table_rename_profile(
            table,
            new_name,
            source::IndexPolicy::Unindexed,
            budget,
            cancelled,
        )
    }

    /// Produce an unpublished rename with qualified unchanged user Int32 indexes.
    ///
    /// System tables remain unindexed; explicit disabled Name AutoCorrect, the
    /// closed single-user-table catalog and all other source guards still apply.
    /// Physical aliases, retained tree ownership and complete key/row membership
    /// must qualify. No current journal intent or frontend save accepts this
    /// expanded profile; the original recoverable rename remains unindexed.
    pub fn plan_indexed_table_rename(
        self,
        table: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableRenamePlan, TableRenameError> {
        self.plan_table_rename_profile(
            table,
            new_name,
            source::IndexPolicy::RetainedInt32,
            budget,
            cancelled,
        )
    }

    fn plan_table_rename_profile(
        self,
        table: &str,
        new_name: &str,
        indexes: source::IndexPolicy,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeTableRenamePlan, TableRenameError> {
        let _span = tracing::debug_span!("native.table.rename.plan").entered();
        self.require_standard_capture()?;
        validate_name(new_name, budget, cancelled)?;
        let qualified = source::qualify(&self, table, new_name, indexes, budget, cancelled)?;
        let mut after = super::record_serialization::copy(self.as_bytes(), budget, cancelled)
            .map_err(source::serialization_error)?;

        if qualified.table != new_name {
            budget
                .charge_allocation(new_name.len() as u64, 1)
                .map_err(InspectError::from)?;
            let value = Value::Text(new_name.to_owned());
            let encoded = super::record_serialization::storage::encode_preferred(
                qualified.name_column(),
                &value,
                budget,
                cancelled,
            )
            .map_err(source::serialization_error)?;
            let row = record::replace_name(&self, &qualified, &encoded, budget, cancelled)?;
            page::replace(
                &self,
                &mut after,
                qualified.identity,
                &row,
                budget,
                cancelled,
            )?;
        }
        source::verify(
            &self,
            &after,
            qualified.identity,
            &qualified.table,
            new_name,
            budget,
            cancelled,
        )?;
        check(budget, cancelled)?;
        Ok(NativeTableRenamePlan {
            before: self,
            after,
            table: qualified.table,
            new_name: qualified.proposed,
            identity: qualified.identity,
        })
    }
}

fn validate_name(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), TableRenameError> {
    check(budget, cancelled)?;
    if name.is_empty() || name.starts_with(' ') {
        return Err(TableRenameError::InvalidName);
    }
    let mut units = 0;
    for character in name.chars() {
        check(budget, cancelled)?;
        units += character.len_utf16();
        if character.is_control()
            || matches!(character, '.' | '!' | '`' | '[' | ']')
            || units > MAX_NAME_UNITS
        {
            return Err(TableRenameError::InvalidName);
        }
    }
    Ok(())
}

fn copy_text(
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, InspectError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(text.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, text.len() as u64)
        .map_err(InspectError::from)?;
    let mut result = String::new();
    result
        .try_reserve_exact(text.len())
        .map_err(|_| InspectError::Allocation)?;
    result.push_str(text);
    Ok(result)
}
