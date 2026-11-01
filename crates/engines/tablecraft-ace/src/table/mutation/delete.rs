//! Explicit cardinality-changing engineering candidates; no save authorization.
mod batch;
mod cascade;
mod patches;
mod relationships;
mod selection;

use super::requests::{RowDelete, TableRowDeletes};
use super::{EditError, NativeEditPlan, rows};
use crate::NativeImage;
use tablecraft_core::limits::ResourceBudget;

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
