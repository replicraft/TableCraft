use crate::{FixedScalarUpdate, RowDelete, RowInsert, XmlByteSpan};
use tablecraft_core::ids::ObjectId;

/// Explicit property selection in the captured table's original Extra metadata.
/// Matching is exact; this does not detect native macro storage or dispatch order.
#[derive(Clone, Copy)]
pub struct BeforeEventGuardSelector<'a> {
    group: u16,
    name: &'a str,
}

impl<'a> BeforeEventGuardSelector<'a> {
    /// Select a unique exact native group-kind/property-name pair.
    pub const fn new(group: u16, name: &'a str) -> Self {
        Self { group, name }
    }

    /// Exact native group kind, without semantic reinterpretation.
    pub const fn group(self) -> u16 {
        self.group
    }

    /// Exact stored name; case/Unicode normalization is not inferred.
    pub const fn name(self) -> &'a str {
        self.name
    }
}

/// Existing exact-carrier intents for explicit before-event guard preparation.
/// This does not request default generation, cascades or a writable candidate.
#[derive(Clone, Copy)]
pub enum BeforeEventIntents<'a> {
    /// All fields explicitly supplied, in native order; no generated columns.
    Insert(&'a [RowInsert]),
    /// One ordinary Int32 field; original expectations remain authoritative.
    FixedUpdate {
        /// Exact source field selector.
        column: &'a str,
        /// Original snapshot-local row expectations and replacements.
        updates: &'a [FixedScalarUpdate],
    },
    /// Exact original record payload expectations.
    Delete(&'a [RowDelete]),
    /// Original live ordinals from this same captured image, without persistent
    /// identity or a raw-byte expectation claim. The engine resolves records.
    DeleteOrdinals(&'a [usize]),
}

/// Explicit event context; not inferred from a property name or XML namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeforeEvent {
    /// All-explicit creation under the BeforeChange event.
    Insert,
    /// Existing-row replacement under the BeforeChange event.
    Update,
    /// Deletion under the documented Old-only BeforeDelete context.
    Delete,
}

impl BeforeEventIntents<'_> {
    pub(super) fn event(self) -> BeforeEvent {
        match self {
            Self::Insert(_) => BeforeEvent::Insert,
            Self::FixedUpdate { .. } => BeforeEvent::Update,
            Self::Delete(_) | Self::DeleteOrdinals(_) => BeforeEvent::Delete,
        }
    }
}

/// Payload-free execution trace kind; indices are preparation-local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeforeGuardTraceKind {
    /// Every selected body was compiled before evaluating any row.
    CompiledAll,
    /// Enter one original ordinal, or one explicit insert request ordinal.
    Begin,
    /// A Boolean condition was evaluated; NULL/nonBoolean never becomes false.
    Condition(bool),
    /// An applicable row finished without raising.
    End,
    /// RaiseError rejected the operation; later statements/rows were not visited.
    Raised(i32),
    /// All applicable rows finished without raising.
    Passed,
    /// A completed RaiseError rejection, never a partial writable result.
    Rejected,
}

/// One visited operation/source node, without expression or row values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeforeGuardTrace {
    pub(super) kind: BeforeGuardTraceKind,
    pub(super) row: Option<usize>,
    pub(super) program: Option<usize>,
    pub(super) node: Option<usize>,
    pub(super) span: Option<XmlByteSpan>,
}

impl BeforeGuardTrace {
    /// Executed step kind.
    pub const fn kind(self) -> BeforeGuardTraceKind {
        self.kind
    }

    /// Original live ordinal or explicit insert request ordinal, when applicable.
    pub const fn row(self) -> Option<usize> {
        self.row
    }

    /// Selected native property ordinal in this preparation.
    pub const fn program(self) -> Option<usize> {
        self.program
    }

    /// Node index within that property's inert source program.
    pub const fn node(self) -> Option<usize> {
        self.node
    }

    /// Original decoded XML UTF-8 source range, never a native file offset.
    pub const fn source_span(self) -> Option<XmlByteSpan> {
        self.span
    }
}

/// Explicit evaluated rejection; no Debug/Display prints its description.
#[derive(PartialEq, Eq)]
pub struct BeforeGuardRejection {
    pub(super) number: i32,
    pub(super) description: Option<String>,
}

impl BeforeGuardRejection {
    /// Checked signed Long result.
    pub const fn number(&self) -> i32 {
        self.number
    }

    /// Explicit Description value; absent differs from present-empty.
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Completed explicit guard preparation; this is never a save capability.
/// Source bytes remain borrowed from the captured image, not a replay token.
pub struct BeforeEventGuardPreparation<'a> {
    pub(super) source: &'a [u8],
    pub(super) event: BeforeEvent,
    pub(super) trace: Vec<BeforeGuardTrace>,
    pub(super) rejection: Option<BeforeGuardRejection>,
    pub(super) object: ObjectId,
    pub(super) programs: Vec<BeforeGuardProgramRevision>,
}

impl BeforeEventGuardPreparation<'_> {
    /// Immutable complete source used for program and row preparation.
    pub fn source_bytes(&self) -> &[u8] {
        self.source
    }

    /// Explicit source-bound event context.
    pub const fn event(&self) -> BeforeEvent {
        self.event
    }

    /// Completed visited order, buffered through the final cancellation check.
    pub fn trace(&self) -> &[BeforeGuardTrace] {
        &self.trace
    }

    /// Evaluated RaiseError rejection, if any; None means only this guard subset passed.
    pub fn rejection(&self) -> Option<&BeforeGuardRejection> {
        self.rejection.as_ref()
    }

    /// Original scoped table identity, never selected from XML names.
    pub const fn table_object_id(&self) -> ObjectId {
        self.object
    }

    /// Exact selected property witnesses bound to the borrowed original image.
    pub fn programs(&self) -> &[BeforeGuardProgramRevision] {
        &self.programs
    }
}

/// Native property identity and retained source bytes, not a reusable permit.
#[derive(PartialEq, Eq)]
pub struct BeforeGuardProgramRevision {
    pub(super) group: u16,
    pub(super) group_ordinal: usize,
    pub(super) property: usize,
    pub(super) name: String,
    pub(super) catalog_page: u32,
    pub(super) catalog_slot: u8,
    pub(super) raw: Vec<u8>,
}

impl BeforeGuardProgramRevision {
    /// Original native group kind.
    pub const fn group(&self) -> u16 {
        self.group
    }

    /// Original group ordinal; duplicate group kinds are not collapsed.
    pub const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    /// Exact selected source property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Property ordinal within its original group.
    pub const fn property(&self) -> usize {
        self.property
    }

    /// Logical catalog root page and row slot.
    pub const fn catalog_reference(&self) -> (u32, u8) {
        (self.catalog_page, self.catalog_slot)
    }

    /// Exact original UTF-16 property value bytes.
    pub fn native_property_bytes(&self) -> &[u8] {
        &self.raw
    }
}
