use super::{COPY_CHUNK_BYTES, ObjectDependency};
use crate::{
    CatalogInspection, Header, InspectError, header::check, inspect_catalog, inspect_header,
};
use std::io::{Cursor, Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

/// Explicit complete-image envelope for a fixed-update-purpose capture.
///
/// This is an image-retention bound, not a memory budget or native format maximum.
/// Capture under this policy currently permits inert reads only: every mutation
/// and normal edit plan remains refused, including on files below 64 MiB.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedUpdateImageLimits {
    max_bytes: u64,
}

impl FixedUpdateImageLimits {
    /// Largest explicitly admitted complete source in the current profile.
    pub const MAX_BYTES: u64 = 80 * 1024 * 1024;

    /// Validate a positive caller ceiling without changing default capture.
    pub const fn new(max_bytes: u64) -> Result<Self, InspectError> {
        if max_bytes == 0 || max_bytes > Self::MAX_BYTES {
            return Err(InspectError::StructureLimit);
        }
        Ok(Self { max_bytes })
    }

    /// Maximum source length retained by this explicit capture.
    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }
}

impl Default for FixedUpdateImageLimits {
    fn default() -> Self {
        Self {
            max_bytes: crate::limits::MAX_IMAGE_BYTES,
        }
    }
}

#[derive(Clone, Copy)]
enum CapturePolicy {
    Standard,
    FixedUpdate(FixedUpdateImageLimits),
}

impl CapturePolicy {
    fn max_bytes(self) -> u64 {
        match self {
            Self::Standard => crate::limits::MAX_IMAGE_BYTES,
            Self::FixedUpdate(limits) => limits.max_bytes(),
        }
    }
}

/// An immutable complete native image, retaining opaque objects and unused bytes.
/// Default capture is capped at 64 MiB. Explicit fixed-update-purpose capture
/// retains its own validated ceiling and currently grants no mutation capability.
/// Both remain independent of the larger read-source envelope and caller budgets.
///
/// Capture requires a stable/offline source. It does not acquire native Access
/// locks, qualify database integrity or execute application content. There is no
/// mutable byte accessor: later edit plans must prove their precise write ranges.
/// No Debug/Serde implementation exposes retained database contents.
pub struct NativeImage {
    pub(crate) bytes: Vec<u8>,
    catalog: CatalogInspection,
    capture: CapturePolicy,
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
        Self::capture_with_policy(source, scope, CapturePolicy::Standard, budget, cancelled)
    }

    /// Retain one complete immutable source under an explicit fixed-purpose limit.
    ///
    /// All bytes, catalog decoding, stable-source requirements, paid resources
    /// and cancellation checks are the same as default capture. This policy is
    /// retained privately, including for small inputs; it cannot authorize other
    /// mutations by presenting a larger work budget or matching image length.
    /// The current capture-only profile refuses every mutation and normal plan.
    pub fn capture_for_fixed_updates<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        limits: FixedUpdateImageLimits,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::capture_with_policy(
            source,
            scope,
            CapturePolicy::FixedUpdate(limits),
            budget,
            cancelled,
        )
    }

    fn capture_with_policy<R: Read + Seek>(
        source: &mut R,
        scope: CatalogScope,
        capture: CapturePolicy,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let _span = tracing::debug_span!("native.image.capture").entered();
        let header = inspect_header(source, budget, cancelled)?;
        header.read_profile()?;
        let length = u64::from(header.page_count()) * u64::from(header.page_size());
        if length > capture.max_bytes() {
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
        Ok(Self {
            bytes,
            catalog,
            capture,
        })
    }

    pub(crate) fn require_standard_capture(&self) -> Result<(), InspectError> {
        match self.capture {
            CapturePolicy::Standard => Ok(()),
            CapturePolicy::FixedUpdate(_) => Err(InspectError::StructureLimit),
        }
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
