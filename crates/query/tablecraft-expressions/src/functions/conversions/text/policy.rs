//! Named numeric syntax profiles; these are not complete OS locale emulations.

/// Explicit execution-local separators and currency token. No OS setting is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberFormat {
    /// Period decimal, no grouping or currency token.
    Invariant,
    /// Period decimal, comma three-digit groups, optional dollar token.
    EnUs,
    /// Comma decimal, period three-digit groups, optional euro token.
    DeDe,
}
impl NumberFormat {
    /// Resolve a supported ASCII-insensitive profile name without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        [
            ("invariant", Self::Invariant),
            ("en-us", Self::EnUs),
            ("de-de", Self::DeDe),
        ]
        .into_iter()
        .find_map(|(spelling, format)| name.eq_ignore_ascii_case(spelling).then_some(format))
    }
    /// Decimal separator for input and explicit numeric output.
    pub const fn decimal_separator(self) -> char {
        if matches!(self, Self::DeDe) { ',' } else { '.' }
    }
    pub(crate) const fn group(self) -> Option<char> {
        match self {
            Self::Invariant => None,
            Self::EnUs => Some(','),
            Self::DeDe => Some('.'),
        }
    }
    pub(crate) const fn currency(self) -> Option<char> {
        match self {
            Self::Invariant => None,
            Self::EnUs => Some('$'),
            Self::DeDe => Some('€'),
        }
    }
}
