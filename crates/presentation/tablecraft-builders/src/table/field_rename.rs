//! Read-only name-shape facts shared by application review and presentation.
use super::MAX_DRAFT_NAME_UNITS;

/// A source observation, not a native capability or promise of physical space.
/// Every proposed rename still needs a fresh consuming native review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRenameNameProfile {
    /// Only an ASCII replacement of the observed original width is qualified.
    SameWidthAscii { units: usize },
    /// ASCII growth/shrink is reviewable; native packing may still refuse it.
    FittedAscii { original_units: usize },
}

impl FieldRenameNameProfile {
    /// Actual original UTF-16 width, separate from the replacement width.
    pub const fn original_units(self) -> usize {
        match self {
            Self::SameWidthAscii { units } => units,
            Self::FittedAscii { original_units } => original_units,
        }
    }

    /// Reject inconsistent observations before attaching them to a field editor.
    pub fn matches_original(self, original: &str) -> bool {
        let units = self.original_units();
        original.is_ascii()
            && (1..=MAX_DRAFT_NAME_UNITS).contains(&units)
            && original.len() == units
    }

    /// Check only the inspected name shape, never native validity or admission.
    pub fn accepts_shape(self, original: &str, proposed: &str) -> bool {
        self.matches_original(original)
            && proposed.is_ascii()
            && (1..=MAX_DRAFT_NAME_UNITS).contains(&proposed.len())
            && match self {
                Self::SameWidthAscii { units } => proposed.len() == units,
                Self::FittedAscii { .. } => true,
            }
    }
}
