//! Catalog-only object discovery without query reconstruction or user rows.
use super::NativeBrowser;
use tablecraft_ace::{
    CatalogInspection, InspectError, NativeObjectOrigin, NativeReadSource, NativeTableStorage,
    inspect_catalog,
};
use tablecraft_model::{ObjectDescriptor, ObjectKind};

/// Complete inert native catalog, preserving object identity and captured order.
/// Presence does not establish readable records or executable query behavior.
pub struct ObjectInventory {
    catalog: CatalogInspection,
}

impl ObjectInventory {
    /// All descriptors and native origins, including unrecognized object kinds.
    /// No table records, saved-query fragments or external sources are opened.
    pub fn entries(&self) -> impl Iterator<Item = ObjectEntry<'_>> {
        self.catalog
            .catalog()
            .objects()
            .iter()
            .zip(self.catalog.origins())
            .map(|(object, origin)| ObjectEntry { object, origin })
    }

    /// Table entries, including system and linked-table declarations.
    pub fn tables(&self) -> impl Iterator<Item = ObjectEntry<'_>> {
        self.entries()
            .filter(|entry| entry.object.kind() == ObjectKind::Table)
    }

    /// Saved-query entries; no definition is reconstructed or evaluated.
    pub fn queries(&self) -> impl Iterator<Item = ObjectEntry<'_>> {
        self.entries()
            .filter(|entry| entry.object.kind() == ObjectKind::Query)
    }
}

/// One original descriptor and its native flags/storage observation.
pub struct ObjectEntry<'a> {
    object: &'a ObjectDescriptor,
    origin: &'a NativeObjectOrigin,
}

impl<'a> ObjectEntry<'a> {
    /// Full scoped identity, original name and common kind. Do not truncate IDs.
    pub const fn object(&self) -> &'a ObjectDescriptor {
        self.object
    }

    /// Declared table storage; None for non-tables or an unclassified declaration.
    /// Local does not establish supported columns or readable rows.
    pub const fn storage(&self) -> Option<NativeTableStorage> {
        self.origin.table_storage()
    }

    /// The original native catalog system-flag observation, without interpreting
    /// names, query bodies or who created the object.
    pub const fn is_system(&self) -> bool {
        self.origin.is_system()
    }
}

impl<R: NativeReadSource> NativeBrowser<'_, R> {
    /// Capture one catalog inventory without reading user records or query bodies.
    /// The browser's original native budget and cancellation callback are retained.
    pub fn inventory(&mut self) -> Result<ObjectInventory, InspectError> {
        let catalog = inspect_catalog(
            &mut self.source,
            self.scope,
            &mut self.budget,
            self.cancelled,
        )?;
        Ok(ObjectInventory { catalog })
    }
}
