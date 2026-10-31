use super::super::{BeforeGuardProgramRevision, BeforeGuardRejection, BeforeGuardTrace};
use crate::XmlByteSpan;
use tablecraft_core::{ids::ObjectId, values::Value};

/// One complete logical final row, never a physical write or persistent handle.
#[derive(PartialEq, Eq)]
pub struct BeforeChangeFinalRow {
    pub(super) ordinal: usize,
    pub(super) values: Vec<Value>,
}

impl BeforeChangeFinalRow {
    /// Original live ordinal, or explicit insert request ordinal.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// All ordinary fields in the original native schema order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

/// One executed SetField location without its expression or assigned value.
#[derive(PartialEq, Eq)]
pub struct BeforeChangeAssignment {
    pub(super) ordinal: usize,
    pub(super) column: usize,
    pub(super) program: usize,
    pub(super) node: usize,
    pub(super) span: XmlByteSpan,
    pub(super) trace_position: usize,
}

impl BeforeChangeAssignment {
    /// Original live ordinal, or explicit insert request ordinal.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Bound position in the original native schema.
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Selected program ordinal in caller order.
    pub const fn program(&self) -> usize {
        self.program
    }

    /// Original source program node.
    pub const fn node(&self) -> usize {
        self.node
    }

    /// Original decoded XML UTF-8 coordinates, never database offsets.
    pub const fn source_span(&self) -> XmlByteSpan {
        self.span
    }

    /// Number of ordinary trace entries preceding this assignment. Equal
    /// positions retain this assignment list's execution order.
    pub const fn trace_position(&self) -> usize {
        self.trace_position
    }
}

/// Explicit BeforeChange logical preparation. This is not required-event
/// coverage, native dispatch, a writable candidate or a save capability.
pub struct BeforeChangeFieldPreparation<'a> {
    pub(super) source: &'a [u8],
    pub(super) object: ObjectId,
    pub(super) trace: Vec<BeforeGuardTrace>,
    pub(super) assignments: Vec<BeforeChangeAssignment>,
    pub(super) rejection: Option<BeforeGuardRejection>,
    pub(super) rows: Option<Vec<BeforeChangeFinalRow>>,
    pub(super) programs: Vec<BeforeGuardProgramRevision>,
}

impl BeforeChangeFieldPreparation<'_> {
    /// Original immutable captured source, not an authorization token.
    pub fn source_bytes(&self) -> &[u8] {
        self.source
    }

    /// Original scoped table identity.
    pub const fn table_object_id(&self) -> ObjectId {
        self.object
    }

    /// Completed control-flow trace; SetField locations are in assignments().
    pub fn trace(&self) -> &[BeforeGuardTrace] {
        &self.trace
    }

    /// Payload-free SetField locations, including those discarded on rejection.
    pub fn assignments(&self) -> &[BeforeChangeAssignment] {
        &self.assignments
    }

    /// Completed RaiseError outcome, if any.
    pub fn rejection(&self) -> Option<&BeforeGuardRejection> {
        self.rejection.as_ref()
    }

    /// Complete final rows on success. None means atomic rejection; Some(empty)
    /// means that every program compiled for an operation with no selected rows.
    pub fn final_rows(&self) -> Option<&[BeforeChangeFinalRow]> {
        self.rows.as_deref()
    }

    /// Exact original program/property witnesses, without save permission.
    pub fn programs(&self) -> &[BeforeGuardProgramRevision] {
        &self.programs
    }
}
