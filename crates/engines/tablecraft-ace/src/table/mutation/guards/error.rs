use crate::{EditError, InspectError};
use tablecraft_expressions::EvalError;
use thiserror::Error;

/// Explicit caller-selection failures, with no names or native payloads exposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeforeGuardSelectionError {
    /// No source-owned program was explicitly selected.
    Empty,
    /// One exact group-kind/name pair has no source match.
    Missing,
    /// The caller selected the same exact pair more than once.
    Duplicate,
    /// More than one native property matches the selected pair.
    Ambiguous,
}

/// Unsupported portions of the explicitly selected guard preparation profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeforeGuardUnsupported {
    /// Unqualified event/field/qualifier or no selected event definition.
    Context,
    /// Opaque body, unsupported action or unknown native program carrier.
    Structure,
    /// Runtime/external call or another unqualified expression form.
    Expression,
    /// Source table/carrier outside the initial all-explicit Int32 profile.
    SourceProfile,
}

/// Guard preparation failures expose no SQL, XML, names or row values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BeforeEventGuardError {
    /// Explicit program selection is absent, stale, repeated or ambiguous.
    #[error("before-event property selection failed ({0:?})")]
    Selection(BeforeGuardSelectionError),
    /// Native source inspection, resources or cancellation.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Exact original intent or ordinary-record binding failed.
    #[error("before-event source intent does not qualify")]
    Intent(#[source] EditError),
    /// Context/structure/expression capability is not implemented.
    #[error("before-event guard profile is unsupported ({0:?})")]
    Unsupported(BeforeGuardUnsupported),
    /// An extracted expression is not one closed admitted expression.
    #[error("before-event expression syntax is invalid")]
    Expression,
    /// Pure scalar evaluation failed; no completed report is returned.
    #[error("before-event scalar evaluation failed")]
    Evaluation(#[source] EvalError),
    /// If result was NULL or another carrier outside the Boolean-only policy.
    #[error("before-event condition must return Boolean")]
    ConditionType,
    /// RaiseError Number/Description did not return the explicit Long/Text profile.
    #[error("before-event action argument has an unsupported carrier")]
    ArgumentType,
    /// Rebinding the original source/intents did not reproduce staged results.
    #[error("before-event preparation authority check failed")]
    Preservation,
}

pub(super) fn evaluation(error: EvalError) -> BeforeEventGuardError {
    match error {
        EvalError::Cancelled => InspectError::Cancelled.into(),
        EvalError::Limit(error) => InspectError::Limit(error).into(),
        EvalError::Allocation => InspectError::Allocation.into(),
        EvalError::DepthLimit | EvalError::TextLimit => InspectError::StructureLimit.into(),
        error => BeforeEventGuardError::Evaluation(error),
    }
}

pub(super) fn intent(error: EditError) -> BeforeEventGuardError {
    match error {
        EditError::Inspect(error) => error.into(),
        error => BeforeEventGuardError::Intent(error),
    }
}
