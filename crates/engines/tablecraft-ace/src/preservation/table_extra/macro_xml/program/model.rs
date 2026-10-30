use super::super::super::TableExtraProperty;
use super::super::DataMacroXml;
use crate::InspectError;

/// Caller-selected structural grammar, not a detected native execution mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroProgramProfile {
    /// Documented desktop Statements/If/RaiseError shapes; expressions unevaluated.
    DocumentedDesktop,
}

/// Structural coverage never authorizes execution, macro absence or writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroProgramCoverage {
    /// Every retained body shape belongs to the selected documented subset.
    RecognizedDesktopSubset,
    /// Well-formed structure outside that subset is retained as opaque source.
    ContainsOpaqueStructure,
}

/// Why a complete subtree stays opaque rather than being silently discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroProgramDiagnostic {
    /// Action name outside the documented subset.
    UnknownAction,
    /// Known action whose context is not qualified by this profile.
    UnsupportedAction,
    /// A documented argument element is not present.
    MissingArgument,
    /// Repeated arguments have no selected first/last interpretation.
    DuplicateArgument,
    /// An unfamiliar argument is retained without interpretation.
    UnknownArgument,
    /// A descendant element belongs to another namespace.
    ForeignElement,
    /// A qualified/foreign attribute prevents structural interpretation.
    ForeignAttribute,
    /// An unfamiliar unqualified attribute is retained.
    UnknownAttribute,
    /// Other well-formed nesting/content outside the initial subset.
    UnsupportedStructure,
}

/// Byte range in the exact original decoded XML UTF-8 input, including BOM.
/// It is not a native UTF-16 property, LvExtra, LVAL or database-file offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XmlByteSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

/// One recognized leaf slot selected without inventing an XML byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroProgramTextKind {
    /// The Condition element's text.
    Condition,
    /// The RaiseError Number argument's text.
    Number,
    /// The optional RaiseError Description argument's text.
    Description,
}

/// Explicitly materialized XML text, without expression evaluation or coercion.
pub struct MacroProgramText {
    pub(super) source: XmlByteSpan,
    pub(super) value: String,
}

impl MacroProgramText {
    /// Original inner-XML range, before entity and line-ending normalization.
    pub const fn source_span(&self) -> XmlByteSpan {
        self.source
    }

    /// Decoded XML 1.0 text; whitespace and expression spelling remain intact.
    pub fn text(&self) -> &str {
        &self.value
    }
}

impl XmlByteSpan {
    /// Inclusive start in the original decoded XML UTF-8 byte buffer.
    pub const fn start(self) -> usize {
        self.start
    }

    /// Exclusive end in that same buffer; present-empty spans have equal ends.
    pub const fn end(self) -> usize {
        self.end
    }
}

/// Recognized structural node; argument and condition contents remain raw XML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroProgramNodeKind {
    /// An ordered statement container, including a present-empty container.
    Statements,
    /// Initial one-If conditional wrapper.
    ConditionalBlock,
    /// An If with a present Condition and Statements element.
    If,
    /// Present condition contents; no Boolean or nonempty-expression claim.
    Condition {
        /// Original inner-XML byte range.
        content: XmlByteSpan,
    },
    /// Desktop RaiseError argument cardinality only; no Long/string evaluation.
    RaiseError {
        /// Present Number contents, which may be empty or an unevaluated expression.
        number: XmlByteSpan,
        /// Optional Description contents; None differs from a present-empty range.
        description: Option<XmlByteSpan>,
    },
    /// Entire uninterpreted subtree remains available through its source span.
    Opaque {
        /// Structural reason, not a claim of globally malformed macro XML.
        diagnostic: MacroProgramDiagnostic,
    },
}

/// One source-bound node in document/preorder order, with snapshot-local indices.
pub struct MacroProgramNode {
    pub(super) definition: usize,
    pub(super) parent: Option<usize>,
    pub(super) span: XmlByteSpan,
    pub(super) kind: MacroProgramNodeKind,
}

impl MacroProgramNode {
    /// Index in this result's unchanged envelope-definition sequence.
    pub const fn definition(&self) -> usize {
        self.definition
    }

    /// Parent node index within this result, never a persistent identity.
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }

    /// Whole original XML element/subtree range.
    pub const fn span(&self) -> XmlByteSpan {
        self.span
    }

    /// Recognized structural shape or explicit opaque diagnostic.
    pub const fn kind(&self) -> MacroProgramNodeKind {
        self.kind
    }
}

/// Inert program inspection borrowing the original native property owner.
/// No Debug/Serde implementation publishes stored program text implicitly.
pub struct DataMacroProgram<'a> {
    pub(super) property: &'a TableExtraProperty,
    pub(super) xml: &'a str,
    pub(super) profile: MacroProgramProfile,
    pub(super) envelopes: DataMacroXml,
    pub(super) nodes: Vec<MacroProgramNode>,
    pub(super) coverage: MacroProgramCoverage,
}

impl DataMacroProgram<'_> {
    /// Explicit caller-selected structural profile, not source-mode autodetection.
    pub const fn profile(&self) -> MacroProgramProfile {
        self.profile
    }

    /// Retained native property bytes, distinct from all XML UTF-8 spans.
    pub fn native_property_bytes(&self) -> &[u8] {
        self.property.raw()
    }

    /// Exact original decoded XML UTF-8 input buffer, including any leading BOM.
    pub fn xml_source(&self) -> &str {
        self.xml
    }

    /// Existing normalized envelope facts; event/name is not an execution permit.
    pub const fn envelopes(&self) -> &DataMacroXml {
        &self.envelopes
    }

    /// Snapshot-local nodes in document order; opaque children are not discarded.
    pub fn nodes(&self) -> &[MacroProgramNode] {
        &self.nodes
    }

    /// Structural status, separate from execution, absence and native write safety.
    pub const fn coverage(&self) -> MacroProgramCoverage {
        self.coverage
    }

    /// Slice the exact original XML byte buffer using this result's coordinates.
    pub fn source_span(&self, span: XmlByteSpan) -> Result<&str, InspectError> {
        self.xml_source()
            .get(span.start..span.end)
            .ok_or(InspectError::Malformed)
    }
}
