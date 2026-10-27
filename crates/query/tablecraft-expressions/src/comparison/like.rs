/// Explicit wildcard-unit and literal-span semantics supplied by a host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeProfile {
    /// Ordering is available, but this provider admits no wildcard matching.
    Unsupported,
    /// Existing Unicode scalar matcher; literals/classes consume one scalar.
    UnicodeScalar,
    /// UTF-16 wildcards and complete weighted literal/element spans.
    Utf16Weights,
}

/// A complete match or a conservative extension bound for a weighted span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LikeSpanState {
    /// Neither this span nor any extension at the same start can match.
    Impossible,
    /// Not a match yet; an extension might match. No existence is promised.
    Possible,
    /// This span matches; extensions might also match.
    Match,
}
