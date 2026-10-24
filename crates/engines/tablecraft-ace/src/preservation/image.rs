use super::{COPY_CHUNK_BYTES, ObjectDependency};
use crate::{
    CatalogInspection, Header, InspectError, header::check, inspect_catalog, inspect_header,
};
use std::io::{Cursor, Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

/// An immutable complete native image, retaining opaque objects and unused bytes.
/// Complete capture is capped at 64 MiB, independently of the larger read-source
/// envelope; caller budgets can impose stricter bounds.
///
/// Capture requires a stable/offline source. It does not acquire native Access
/// locks, qualify database integrity or execute application content. There is no
/// mutable byte accessor: later edit plans must prove their precise write ranges.
/// No Debug/Serde implementation exposes retained database contents.
pub struct NativeImage {
    pub(crate) bytes: Vec<u8>,
    catalog: CatalogInspection,
}
impl NativeImage {
    /// Retain every input byte before inspecting metadata from that frozen image.
    /// Work/read/allocation budgets and cancellation bound capture. A changed
    /// length/header fails, but cannot detect all noncooperating concurrent writes;
    /// the caller must keep the input offline throughout capture.
    pub fn capture<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.image.capture").entered();
        let header = inspect_header(source, budget, cancelled)?;
        header.read_profile()?;
        let length = u64::from(header.page_count()) * u64::from(header.page_size());
        if length > crate::limits::MAX_IMAGE_BYTES {
            return Err(InspectError::StructureLimit);
        }
        budget.charge_allocation(length, 1)?;
        budget.charge(Resource::Bytes, length)?;
        let size = usize::try_from(length).map_err(|_| InspectError::StructureLimit)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| InspectError::Allocation)?;
        bytes.resize(size, 0);
        for (index, chunk) in bytes.chunks_mut(COPY_CHUNK_BYTES).enumerate() {
            tablecraft_io::read_exact_at(
                source,
                (index * COPY_CHUNK_BYTES) as u64,
                chunk,
                budget,
                cancelled,
            )?;
        }
        check(budget, cancelled)?;
        if tablecraft_io::seek_to_end(source, budget, cancelled)? != length
            || Header::parse(&bytes, length)? != header
        {
            return Err(InspectError::Malformed);
        }
        let catalog = inspect_catalog(&mut Cursor::new(&bytes), scope, budget, cancelled)?;
        Ok(Self { bytes, catalog })
    }

    /// Complete original bytes, including bodies this engine cannot decode.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Inert catalog decoded only from the captured bytes.
    pub fn catalog(&self) -> &CatalogInspection {
        &self.catalog
    }

    /// Independent read cursor without copying or exposing mutable bytes.
    pub fn reader(&self) -> Cursor<&[u8]> {
        Cursor::new(&self.bytes)
    }

    /// Conservative per-object dependency coverage without inspecting bodies.
    /// Uninspected objects cannot be treated as free of VBA or data macros.
    pub fn dependencies(&self) -> impl Iterator<Item = ObjectDependency<'_>> {
        self.catalog
            .catalog()
            .objects()
            .iter()
            .map(|object| ObjectDependency { object })
    }
}
