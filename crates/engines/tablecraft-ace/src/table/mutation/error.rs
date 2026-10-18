use crate::InspectError;
use thiserror::Error;

/// A required write capability outside the implemented offline profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteBlocker {
    /// Only unencoded Jet 4 writes are qualified; ACE data macros are uninspected.
    Format,
    /// System, hidden, replicated or otherwise flagged native table.
    Table,
    /// Non-fixed/non-Int32, AutoNumber, extended or overlapping storage.
    Column,
    /// Updating an indexed field requires unimplemented index mutation.
    IndexedColumn,
    /// Property semantics outside the explicitly handled/passive keys.
    Properties,
    /// Opaque or uninspected catalog payloads may contain unavailable behavior.
    ActiveContent,
    /// A field, relationship or index check has unsupported coverage.
    ConstraintCoverage,
    /// Relocated records or historical column layouts need a separate writer.
    RowLayout,
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
    /// An edit changed a byte outside its validated field payloads.
    #[error("native edit preservation proof failed")]
    Preservation,
}
