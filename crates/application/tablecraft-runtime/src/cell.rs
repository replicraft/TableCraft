//! Typed read-only presentation cells; no child payload or executable action.

use tablecraft_display::ScalarPresentation;

/// Default type-directed presentation; stored TextAlign overrides are uninspected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlignment {
    #[default]
    Left,
    Right,
}

/// One complete display/copy value and its independently supplied semantics.
/// Collection text is a summary, not an export of the collection's contents.
pub struct Cell {
    pub presentation: Option<ScalarPresentation>,
    pub alignment: HorizontalAlignment,
    pub text: String,
    pub kind: CellKind,
}

/// Observed native field-control declaration, independent of value type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayControl {
    /// The native property carrier was not inspected, including query results.
    Uninspected,
    /// The inspected field has no DisplayControl property.
    Absent,
    /// Exact native signed 16-bit control code, without a fabricated default.
    Native(i16),
    /// The property exists with unqualified flags, type or width.
    Unsupported,
}

/// Reader facts, kept separate from the displayed text. A literal text "NULL"
/// is an ordinary value; it cannot acquire NULL semantics from its spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellKind {
    /// This selected child member was deliberately not inspected; never SQL NULL.
    Uninspected,
    /// An outer-joined result has no parent on this side; never an empty collection.
    AbsentCollection,
    /// Stored calculations are cached results with unestablished freshness.
    Scalar {
        is_null: bool,
        stored_calculation: bool,
        /// Exact typed Boolean value; display text never establishes this fact.
        boolean: Option<bool>,
        display_control: DisplayControl,
    },
    /// Complete checked stored membership, including duplicates and NULL items.
    Multivalue { assigned: bool, members: usize },
    /// Complete child membership; NULL FileData still counts as an attachment.
    /// Payloads remain encoded and are not included in this summary DTO.
    Attachments { assigned: bool, members: usize },
    /// Stored versions in their native order, separate from visible current Memo.
    VersionHistory { assigned: bool, members: usize },
}

impl Cell {
    pub(crate) fn expression(text: String) -> Self {
        Self {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text,
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: None,
                display_control: DisplayControl::Uninspected,
            },
        }
    }
}
