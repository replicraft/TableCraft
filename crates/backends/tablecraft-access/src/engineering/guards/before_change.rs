//! One explicit SQL-selected BeforeChange fitting engineering candidate.
use super::super::{AccessEngineeringEditor, ScalarUpdateMode, UpdateMode, sql};
use crate::NativeEditError;
use tablecraft_ace::{
    Ace16BeforeChangePreview, BeforeChangePreviewError, BeforeEventGuardSelector, NativeImage,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, UpdateProfile,
};
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::{UpdateError, UpdatePlan};
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Source snapshot conversion or native BeforeChange preparation refused.
/// A rejection/error returns no candidate and never publishes a partial result.
#[derive(Debug, Error)]
pub enum BeforeChangeBackendError {
    /// Original snapshot, handle, carrier or expected value qualification failed.
    #[error(transparent)]
    Native(#[from] NativeEditError),
    /// Native complete-program preparation or the fitting physical update failed.
    #[error(transparent)]
    Preview(#[from] BeforeChangePreviewError),
}

/// Resolve one local one-column UPDATE through the existing shared planner,
/// then execute only the explicitly borrowed property selectors and finish one
/// fitting candidate. Caller selector order is engineering order, not native
/// event discovery/dispatch or required-program coverage. No defaults, generation,
/// cascades, journal/save authority or publication is introduced.
pub fn preview_update(
    image: NativeImage,
    query: &Query,
    selectors: &[BeforeEventGuardSelector<'_>],
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<Ace16BeforeChangePreview>, UpdateError<BeforeChangeBackendError>> {
    let _span = tracing::debug_span!("access.engineering.before_change.preview_update").entered();
    let table = sql::local_update_table(query)?;
    let editor = AccessEngineeringEditor::new(
        image,
        table,
        ScalarUpdateMode::Fixed(UpdateMode::Fitted),
        budget,
        cancelled,
    )
    .map_err(BeforeChangeBackendError::Native)
    .map_err(UpdateError::Backend)?;

    tablecraft_query::plan_update(
        BeforeChangeEditor {
            inner: editor,
            selectors,
        },
        query,
        budget,
        collation,
        cancelled,
    )
}

/// Resolve a local original-row multi-target UPDATE, then run explicitly selected
/// BeforeChange programs once per original affected row and finish once.
/// This additive rich-schema service has no mode/flag or fallback. User targets
/// are the qualified preserved fixed carriers; native macro operands/SetField
/// retain the Int32 profile. Discovery, normal saves and publication stay separate.
pub fn preview_batch_update(
    image: NativeImage,
    query: &Query,
    selectors: &[BeforeEventGuardSelector<'_>],
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<Ace16BeforeChangePreview>, UpdateError<BeforeChangeBackendError>> {
    let _span =
        tracing::debug_span!("access.engineering.before_change.preview_batch_update").entered();
    let table = sql::local_batch_update_table(query)?;
    let editor = BeforeChangeBatchEditor::new(image, table, selectors, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    tablecraft_query::plan_batch_update(editor, query, budget, collation, cancelled)
}

/// Snapshot-bound typed rich fixed batch. Supplied selectors remain borrowed,
/// exact and explicit; no inventory entry is chosen automatically. The completed
/// physical result owns trace/candidate bytes, never partial logical final rows.
pub struct BeforeChangeBatchEditor<'selectors> {
    inner: AccessEngineeringEditor,
    selectors: &'selectors [BeforeEventGuardSelector<'selectors>],
}

impl<'selectors> BeforeChangeBatchEditor<'selectors> {
    /// Capture the existing real engineering snapshot and retain exact caller
    /// selectors. Complete program/write eligibility is established only when
    /// the consuming typed batch is finished by the native service.
    pub fn new(
        image: NativeImage,
        table: &str,
        selectors: &'selectors [BeforeEventGuardSelector<'selectors>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, BeforeChangeBackendError> {
        let inner = AccessEngineeringEditor::new(
            image,
            table,
            ScalarUpdateMode::Default,
            budget,
            cancelled,
        )?;
        inner.require_before_change_batch_profile()?;
        Ok(Self { inner, selectors })
    }
}

impl EditableTable for BeforeChangeBatchEditor<'_> {
    type Error = BeforeChangeBackendError;
    type Plan = Ace16BeforeChangePreview;

    fn snapshot(&self) -> &Snapshot {
        self.inner.snapshot()
    }

    fn update_profile(&self) -> UpdateProfile {
        self.inner.update_profile()
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner.preview_before_change_batch(
            self.selectors,
            &[ColumnUpdates { column, updates }],
            budget,
            cancelled,
        )
    }
}

impl EditableBatch for BeforeChangeBatchEditor<'_> {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .preview_before_change_batch(self.selectors, columns, budget, cancelled)
    }
}

struct BeforeChangeEditor<'a> {
    inner: AccessEngineeringEditor,
    selectors: &'a [BeforeEventGuardSelector<'a>],
}

impl EditableTable for BeforeChangeEditor<'_> {
    type Error = BeforeChangeBackendError;
    type Plan = Ace16BeforeChangePreview;

    fn snapshot(&self) -> &Snapshot {
        self.inner.snapshot()
    }

    fn update_profile(&self) -> UpdateProfile {
        self.inner.update_profile()
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        self.inner
            .preview_before_change(self.selectors, column, updates, budget, cancelled)
    }
}
