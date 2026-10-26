//! Explicit cardinality-changing engineering candidates; no save authorization.
mod batch;
mod patches;
mod relationships;

use super::{EditError, NativeEditPlan, rows};
use crate::NativeImage;
use tablecraft_core::limits::ResourceBudget;

/// One live ordinal in the exact captured image, with its complete raw payload.
///
/// Expected bytes start with the stored column-count header and end with the
/// final NULL/Boolean mask. They exclude the slot word and page header. They are
/// compared with the checked ordinary record, without decoding/coercing values.
/// Ordinals are snapshot-local, not persistent record identities. No Debug
/// implementation exposes record contents.
pub struct RowDelete {
    row: usize,
    expected: Vec<u8>,
}

impl RowDelete {
    /// Own an expected record; actual traversal validates its bounded framing.
    pub const fn new(row: usize, expected: Vec<u8>) -> Self {
        Self { row, expected }
    }

    /// Sequential live-row ordinal in this captured image only.
    pub const fn row(&self) -> usize {
        self.row
    }

    /// Exact expected ordinary record bytes, excluding slot/page metadata.
    pub fn expected_bytes(&self) -> &[u8] {
        &self.expected
    }
}

impl rows::RowAssignment for RowDelete {
    fn row(&self) -> usize {
        self.row
    }
}

/// Explicit row deletions for one table in the original captured image.
///
/// Empty groups still qualify that table. Table names may not repeat within a
/// batch, including through case-insensitive spellings. No cascade is inferred.
pub struct TableRowDeletes<'a> {
    table: &'a str,
    deletes: &'a [RowDelete],
}

impl<'a> TableRowDeletes<'a> {
    /// Borrow one table name and its snapshot-local raw row requests.
    pub const fn new(table: &'a str, deletes: &'a [RowDelete]) -> Self {
        Self { table, deletes }
    }

    /// Explicit native table name, resolved through the original catalog.
    pub const fn table(&self) -> &'a str {
        self.table
    }

    /// Exact original row requests for this table.
    pub const fn deletes(&self) -> &'a [RowDelete] {
        self.deletes
    }
}

/// Unqualified ACE16 deletion bytes for explicit fresh-copy comparison only.
///
/// Data-macro absence/execution and native Access save/reopen are not qualified.
/// This feature-gated candidate has no journal/apply operation and cannot become
/// a qualified native edit plan. Deleted payloads and long-value storage are not
/// erased or reclaimed; their root slots become tombstones.
///
/// ```compile_fail
/// fn cannot_commit(candidate: tablecraft_ace::Ace16DeletePreview) {
///     let _: tablecraft_ace::NativeEditPlan = candidate;
/// }
/// ```
pub struct Ace16DeletePreview {
    plan: NativeEditPlan,
}

impl Ace16DeletePreview {
    /// Captured original image, unchanged by preview preparation.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete candidate bytes, never written to the source by this service.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Number of live roots removed, including every requested valid ordinal.
    pub const fn deleted_rows(&self) -> usize {
        self.plan.changed_rows()
    }
}

impl NativeImage {
    /// Prepare ordinary-root deletion inside existing ACE16 pages.
    ///
    /// All physical indexes must qualify for the existing write codecs or the
    /// delete-only single Short Text/Short Binary preservation profile. Entries
    /// are removed within their original leaf partitions; ancestor bounds use full
    /// retained key/row maxima. Empty nonroot pages may retire through qualified
    /// existing owned/global bitmap cells. Nonempty nodes retain their heights;
    /// an empty tree reuses its root as a height-zero leaf. Historical counters,
    /// file length and record payloads stay fixed. Missing map representation,
    /// unsafe allocation metadata or a required split refuses the whole candidate.
    /// Only qualified cross-table enforced RESTRICT relationships are admitted;
    /// complete final membership rejects surviving references to deleted parents.
    /// Self references and other delete actions refuse even empty requests.
    /// Existing active content, property, constraint, ignored-NULL membership
    /// and historical-layout gates remain. Normal ACE writes stay blocked;
    /// no macro eligibility is implied.
    pub fn preview_ace16_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16DeletePreview, EditError> {
        self.preview_ace16_batch_deletes(&[TableRowDeletes::new(table, deletes)], budget, cancelled)
    }
}
