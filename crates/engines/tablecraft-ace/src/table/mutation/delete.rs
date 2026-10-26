//! Explicit cardinality-changing engineering candidates; no save authorization.
mod patches;

use super::{EditError, NativeEditPlan, WriteBlocker, gates, preview, rows};
use crate::{
    InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    inspect_relationships,
    table::indexes::index_delete_patches,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

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
    /// All physical indexes must qualify for existing write codecs. Root leaves
    /// may become empty; multi-level trees retain every original leaf partition
    /// and require each leaf to stay nonempty. Exact entries are removed once per
    /// physical index; ancestor bounds use complete retained key/row maxima.
    /// Historical counters, topology, allocation, file length and payloads stay fixed.
    /// A required split or empty non-root leaf refuses the complete candidate.
    /// Declared relationships touching this table are refused, including empty
    /// requests; no delete action or cascade is silently omitted. Existing active
    /// content, property, constraint, ignored-NULL and historical-layout gates
    /// remain. Normal ACE writes stay blocked; no macro eligibility is implied.
    pub fn preview_ace16_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16DeletePreview, EditError> {
        let _span =
            tracing::debug_span!("native.delete.preview", assignments = deletes.len()).entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        let before = gates::before(&self, table, budget, cancelled)?;
        let scope = self.catalog().catalog().scope();
        for relationship in inspect_relationships(&mut self.reader(), scope, budget, cancelled)? {
            check(budget, cancelled)?;
            let declaration = relationship.declaration();
            if declaration
                .parent_table()
                .as_str()
                .eq_ignore_ascii_case(before.schema().name())
                || declaration
                    .child_table()
                    .as_str()
                    .eq_ignore_ascii_case(before.schema().name())
            {
                return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
            }
        }
        rows::unique(deletes, budget, cancelled)?;
        for request in deletes {
            check(budget, cancelled)?;
            if request.expected.len() > PAGE_BYTES {
                return Err(InspectError::StructureLimit.into());
            }
            budget
                .charge(Resource::Steps, request.expected.len() as u64)
                .map_err(InspectError::from)?;
        }
        let mut source = self.reader();
        let mut reader = Reader {
            source: &mut source,
            header: self.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let (records, references) = patches::records(&mut reader, before.schema(), deletes)?;
        let indexes = index_delete_patches(
            &self,
            before.schema(),
            before.indexes(),
            &references,
            reader.budget,
            reader.cancelled,
        )?;
        self.finish_updates(table, &records, &indexes, deletes.len(), budget, cancelled)
            .map(|plan| Ace16DeletePreview { plan })
    }
}
