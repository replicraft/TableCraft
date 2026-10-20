use super::{origin::NativeObjectOrigin, record};
use crate::format::{allocation, bytes, layout, reader};
use crate::{Header, InspectError, inspect_header};
use crate::{
    format::{
        page::{DATA_SIGNATURE, OWNER_OFFSET},
        row::{DELETED_MASK, slot_offset},
    },
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::{CatalogScope, ObjectCatalog, ObjectDescriptor};
use tablecraft_schema::{NameComparison, SchemaName};

const CATALOG_DEFINITION_PAGE: u32 = 2;
const NO_PARENT_ID: i32 = 0;
// Conservative per-object allowance for the model catalog's identity HashSet.
const ID_INDEX_BYTES_PER_OBJECT: usize = 64;

/// Completed inert header/catalog inventory. Object bodies remain uninspected.
pub struct CatalogInspection {
    header: Header,
    catalog: ObjectCatalog,
    origins: Vec<NativeObjectOrigin>,
}
impl CatalogInspection {
    /// Header metadata for this source.
    pub const fn header(&self) -> Header {
        self.header
    }
    /// Shared metadata inventory, including unknown kinds and unresolved parents.
    pub fn catalog(&self) -> &ObjectCatalog {
        &self.catalog
    }
    /// Native references in the same order as catalog objects.
    pub fn origins(&self) -> &[NativeObjectOrigin] {
        &self.origins
    }
}
/// Inspect MSysObjects metadata in a qualified plain or explicitly decoded stable
/// source (Jet 4 or ACE codes 2/3/5/6). Other layouts and unsupported structures
/// fail explicitly. Nothing is executed and no partial inventory is returned.
/// Caller-issued scope identifies this source; never reuse it across unrelated
/// catalogs. IDs are not persisted by this reader. All work uses shared budgets.
pub fn inspect_catalog<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CatalogInspection, InspectError> {
    let _span = tracing::debug_span!("native.catalog.read").entered();
    let header = inspect_header(source, budget, cancelled)?;
    let header = source.qualify_header(header)?;
    header.read_profile()?;
    let mut reader = reader::Reader {
        source,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let layout = layout::read(&mut reader, CATALOG_DEFINITION_PAGE)?;
    layout::validate_catalog(&layout)?;
    if layout.records as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let pages = allocation::pages(&mut reader, layout.usage)?;
    let mut objects = Vec::new();
    let mut origins = Vec::new();
    for page_number in pages {
        let page = reader.page(page_number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || bytes::u32_at(&page, OWNER_OFFSET)? != CATALOG_DEFINITION_PAGE
        {
            return Err(InspectError::Malformed);
        }
        for index in 0..reader::row_count(&page)? {
            reader.tick()?;
            let flags = bytes::u16_at(&page, slot_offset(index))?;
            if flags & DELETED_MASK != 0 {
                continue;
            }
            if objects.len() >= MAX_LIVE_ROWS {
                return Err(InspectError::StructureLimit);
            }
            reader.budget.charge(Resource::Rows, 1)?;
            let row =
                reader.row_from_page(page_number, index as u8, CATALOG_DEFINITION_PAGE, &page)?;
            let decoded = record::decode(&row, &layout, reader.budget)?;
            let id = scope.object_id(u64::from(decoded.id as u32));
            let parent = (decoded.parent != NO_PARENT_ID)
                .then(|| scope.object_id(u64::from(decoded.parent as u32)));
            reader.budget.charge_allocation(
                1,
                (std::mem::size_of::<ObjectDescriptor>()
                    + std::mem::size_of::<NativeObjectOrigin>()
                    + ID_INDEX_BYTES_PER_OBJECT) as u64,
            )?;
            objects
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            origins
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            objects.push(ObjectDescriptor::new(
                id,
                SchemaName::new(decoded.name)?,
                record::kind(decoded.kind),
                parent,
            ));
            origins.push(NativeObjectOrigin {
                object: id,
                native_id: decoded.id,
                parent_id: decoded.parent,
                type_code: decoded.kind,
                flags: decoded.flags,
                page: page_number,
                slot: index as u8,
            });
        }
    }
    if objects.len() != layout.records as usize {
        return Err(InspectError::Malformed);
    }
    reader.tick()?;
    let catalog = ObjectCatalog::new(
        scope,
        objects,
        NameComparison::AsciiInsensitive,
        MAX_LIVE_ROWS,
    )?;
    Ok(CatalogInspection {
        header,
        catalog,
        origins,
    })
}
