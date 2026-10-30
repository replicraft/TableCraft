//! Explicit engineering candidates, separate from qualified native edit plans.
use super::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
use crate::{FileFormat, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

const SUPPORTED_PREVIEW_SUBVERSION: u8 = 0;

/// An unqualified ACE 16 physical candidate for independent/native comparisons.
///
/// This is not a database save or executable SQL action: unavailable data macros
/// are neither detected nor executed. Existing structural, property, constraint,
/// direct-child/payload and unchanged-byte checks still apply. Passing them does
/// not establish macro absence or Access compatibility. Only disposable offline
/// copies should be materialized. This type has no journal/commit operation and
/// cannot be converted into a qualified [`NativeEditPlan`].
///
/// ```compile_fail
/// fn cannot_commit(preview: tablecraft_ace::Ace16WritePreview) {
///     let _: tablecraft_ace::NativeEditPlan = preview;
/// }
/// ```
pub struct Ace16WritePreview {
    before: Vec<u8>,
    after: Vec<u8>,
    changed_rows: usize,
}

impl Ace16WritePreview {
    pub(super) fn from_plan(plan: NativeEditPlan) -> Self {
        Self {
            before: plan.before,
            after: plan.after,
            changed_rows: plan.changed_rows,
        }
    }
    pub(super) fn from_candidate(before: Vec<u8>, after: Vec<u8>, changed_rows: usize) -> Self {
        Self {
            before,
            after,
            changed_rows,
        }
    }

    /// Original complete captured image, unchanged by preview creation.
    pub fn before_bytes(&self) -> &[u8] {
        &self.before
    }
    /// Unqualified candidate bytes, for explicit fresh-copy comparison only.
    pub fn candidate_bytes(&self) -> &[u8] {
        &self.after
    }
    /// Number of effective changes reported by this operation. INSERT and DELETE
    /// count rows; UPDATE counts changed field assignments, excluding no-ops.
    pub fn changed_rows(&self) -> usize {
        self.changed_rows
    }
}

impl NativeImage {
    /// Preview bounded fixed non-NULL Int32 field/index patches on ACE 16.
    ///
    /// Requires `ace16-write-preview`, disabled by default in the standalone
    /// engine. Explicit Access engineering services and fresh-copy CLI commands
    /// opt in; ordinary save and journal admission stay unchanged.
    /// Only unencoded subversion-zero 2016/2019 read profiles
    /// are eligible. Other physical/constraint/preservation gates are unchanged.
    /// Eligible foreign indexes require complete reciprocal/catalog and before/
    /// after membership checks; required cascades are not executed.
    /// Unknown data-macro behavior remains unqualified; this method grants no
    /// production write capability. It performs no source I/O writes or execution.
    pub fn preview_ace16_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: &[Int32Update],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.preview", assignments = updates.len()).entered();
        check(budget, cancelled)?;
        qualify(&self)?;
        self.plan_int32_updates_inner(table, column, updates, budget, cancelled)
            .map(Ace16WritePreview::from_plan)
    }
}

pub(super) fn qualify(image: &NativeImage) -> Result<(), EditError> {
    image.require_standard_capture()?;
    qualify_format(image)
}

pub(super) fn qualify_fixed(image: &NativeImage) -> Result<(), EditError> {
    image.require_fixed_update_capture()?;
    qualify_format(image)
}

fn qualify_format(image: &NativeImage) -> Result<(), EditError> {
    if !matches!(
        image.catalog().header().format(),
        FileFormat::Ace2016 | FileFormat::Ace2019
    ) || image.catalog().header().subversion() != SUPPORTED_PREVIEW_SUBVERSION
        || image.catalog().header().encoded_pages()
    {
        return Err(EditError::Unsupported(WriteBlocker::Format));
    }
    Ok(())
}
