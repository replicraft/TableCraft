//! Original semantic version2 reconstruction before fixed-journal recovery.
use super::{NativeQueryExpressionEditsIntent, QueryExpressionEditsIntentError};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// Same retained checked V1 journal after complete native request reconstruction.
#[must_use = "recover explicitly or retain the qualified expression-edits journal"]
pub struct QualifiedQueryExpressionEditsRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Rebuild both complete images and actual identity using one cumulative allowance.
/// Neither decoded intent nor syntactically valid journal AFTER grants authority.
pub fn qualify_native_query_expression_edits_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeQueryExpressionEditsIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedQueryExpressionEditsRecovery<'a, J>, QueryExpressionEditsIntentError> {
    let _span = tracing::debug_span!("native.query_expression_edits_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;

    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = intent.reconstruct(image, budget, cancelled)?;
    intent.verify_plan(&plan, budget, cancelled)?;
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedQueryExpressionEditsRecovery { checked })
}

impl<J: DurableStorage> QualifiedQueryExpressionEditsRecovery<'_, J> {
    /// Exact framing state of this retained generic fixed-image journal session.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Consume this same session; no recapture, semantic replay or cleanup occurs.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm a complete settled image without replay or target repair writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
