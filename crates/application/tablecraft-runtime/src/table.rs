//! Inert table catalog facts for application hosts.

/// A catalog entry; local storage does not imply edit eligibility.
pub struct Table {
    pub name: String,
    /// Local storage fact; independent of display text and row readability.
    pub local: bool,
    pub system: bool,
    pub storage: String,
}
