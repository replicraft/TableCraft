//! Native field-addition declarations shared by request data and native producers.

/// Explicit semantic declaration for the separate field-addition candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldAdditionTarget {
    /// Nullable ordinary LONG, Required(false), exact typed DefaultValue zero.
    NumberLongZero,
    /// Nullable Short Text, Required(false), AllowZeroLength(true), no default.
    ShortText {
        /// Explicit native capacity of 1 through 255 UTF-16 code units.
        max_utf16_units: usize,
        /// Explicit Unicode compression permission, without changing old defaults.
        unicode_compression: bool,
    },
}
