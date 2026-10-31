use crate::InspectError;
use tablecraft_expressions::EvalError;
use thiserror::Error;

/// A required write capability outside the implemented offline profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteBlocker {
    /// Format, subversion or encoding outside the selected writer profile.
    Format,
    /// System, hidden, replicated, flagged or aliased native table definition.
    Table,
    /// Field type, generated/extended flags or storage outside this writer profile.
    Column,
    /// Index mutation requires an unqualified layout, prefix or key profile.
    IndexedColumn,
    /// Property semantics outside the explicitly handled/passive keys.
    Properties,
    /// Opaque or uninspected catalog payloads may contain unavailable behavior.
    ActiveContent,
    /// A field, relationship or index check has unsupported coverage.
    ConstraintCoverage,
    /// Relocated records or historical column layouts need a separate writer.
    RowLayout,
    /// Native timing for an explicitly supplied equal assignment remains unqualified.
    #[cfg(feature = "journal")]
    EventTiming,
}

/// Native edit errors omit names, paths and values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditError {
    /// Inspection, validation, cancellation, allocation or resource failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Required write behavior is not implemented for this input.
    #[error("native write profile is unsupported ({0:?})")]
    Unsupported(WriteBlocker),
    /// An update ordinal is absent, repeated or has a different expected value.
    #[error("native update does not match its captured row")]
    Conflict,
    /// A required pure BeforeChange program rejected the complete update.
    #[cfg(feature = "journal")]
    #[error("required BeforeChange rejected the update")]
    BeforeChangeRejected(crate::BeforeGuardRejection),
    /// A supported required BeforeChange expression failed during evaluation.
    #[cfg(feature = "journal")]
    #[error("required BeforeChange evaluation failed")]
    BeforeChangeEvaluation(#[source] EvalError),
    /// A declared RESTRICT action forbids changing a referenced original key.
    #[error("native relationship action restricts this update")]
    ReferentialAction,
    /// The selected incrementing Int32 generator has no remaining values.
    #[error("incrementing AutoNumber range is exhausted")]
    AutoNumberExhausted,
    /// An explicitly authorized default clock or projection failed.
    #[error("native default evaluation failed")]
    DefaultEvaluation(#[source] EvalError),
    /// An edit changed a byte outside its validated field payloads.
    #[error("native edit preservation proof failed")]
    Preservation,
}
