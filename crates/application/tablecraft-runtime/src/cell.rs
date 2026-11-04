//! Typed read-only presentation cells; no child payload or executable action.

/// One complete display/copy value and its independently supplied semantics.
/// Collection text is a summary, not an export of the collection's contents.
pub struct Cell {
    pub text: String,
    pub kind: CellKind,
}

/// Reader facts, kept separate from the displayed text. A literal text "NULL"
/// is an ordinary value; it cannot acquire NULL semantics from its spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellKind {
    /// Stored calculations are cached results with unestablished freshness.
    Scalar {
        is_null: bool,
        stored_calculation: bool,
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
            text,
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
            },
        }
    }
}
