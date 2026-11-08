//! Complete request/source/AFTER reconstruction before checked-journal recovery.
use super::{NativeQueryExpressionIntent, QueryExpressionRecoveryError};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The same retained checked V1 session after complete native query reconstruction.
#[must_use = "recover explicitly or retain the qualified query-expression journal"]
pub struct QualifiedQueryExpressionRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Rebuild both complete images and original query identity before granting recovery.
/// A valid TCQE record or a syntactically valid stored AFTER is never sufficient.
pub fn qualify_native_query_expression_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeQueryExpressionIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedQueryExpressionRecovery<'a, J>, QueryExpressionRecoveryError> {
    let _span = tracing::debug_span!("native.query_expression_recovery.qualify").entered();
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
    Ok(QualifiedQueryExpressionRecovery { checked })
}

impl<J: DurableStorage> QualifiedQueryExpressionRecovery<'_, J> {
    /// Exact framing state established by this retained V1 inspection session.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Consume this same qualified journal to recover the caller-bound target.
    /// No journal recapture, request reinterpretation or namespace cleanup occurs.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm a complete settled current image without replay or repair writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
