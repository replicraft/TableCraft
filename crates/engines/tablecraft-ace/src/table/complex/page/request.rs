//! Checked page ranges and complete collection retention are separate controls.
use crate::{TablePageRequest, TableSnapshotLimits};

/// A stored collection page plus explicit complete parent/child snapshot limits.
/// This is an operation request, not a persistent bookmark or live-file snapshot.
///
/// Keep the returned request when opting into stored calculations:
///
/// ```
/// use tablecraft_ace::{CollectionPageRequest, TablePageRequest, TableSnapshotLimits};
///
/// let page = TablePageRequest::new(0, 1)?;
/// let request = CollectionPageRequest::new(page, TableSnapshotLimits::default())
///     .with_stored_calculations(true);
/// assert!(request.includes_stored_calculations());
/// # Ok::<(), tablecraft_ace::InspectError>(())
/// ```
///
/// Discarding a configured copy is rejected:
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use tablecraft_ace::{CollectionPageRequest, TablePageRequest, TableSnapshotLimits};
///
/// let page = TablePageRequest::new(0, 1)?;
/// let request = CollectionPageRequest::new(page, TableSnapshotLimits::default());
/// request.with_stored_calculations(true);
/// # Ok::<(), tablecraft_ace::InspectError>(())
/// ```
#[must_use = "retain the returned request to apply its collection read settings"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectionPageRequest {
    page: TablePageRequest,
    snapshot_limits: TableSnapshotLimits,
    include_stored_calculations: bool,
}

impl CollectionPageRequest {
    /// Combine validated controls without reading data or enabling formulas.
    pub const fn new(page: TablePageRequest, snapshot_limits: TableSnapshotLimits) -> Self {
        Self {
            page,
            snapshot_limits,
            include_stored_calculations: false,
        }
    }

    /// Include qualified cached scalar results without evaluating their formulas.
    pub const fn with_stored_calculations(mut self, include: bool) -> Self {
        self.include_stored_calculations = include;
        self
    }

    /// Requested visible parent range; full collection checks still precede output.
    pub const fn page(self) -> TablePageRequest {
        self.page
    }

    /// Shared ceiling and internal batch size for complete parent/child tables.
    pub const fn snapshot_limits(self) -> TableSnapshotLimits {
        self.snapshot_limits
    }

    /// Whether inert qualified cached scalar results are included.
    pub const fn includes_stored_calculations(self) -> bool {
        self.include_stored_calculations
    }
}
