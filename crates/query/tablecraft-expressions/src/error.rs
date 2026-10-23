use thiserror::Error;

/// Evaluation failures without SQL, field names or value payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EvalError {
    /// A syntax form has no evaluator in this initial profile.
    #[error("expression form is not implemented")]
    UnsupportedExpression,
    /// The host has no value for the requested field/parameter.
    #[error("expression field has no binding")]
    UnboundField,
    /// Literal syntax cannot be materialized in this numeric profile.
    #[error("literal is outside the supported evaluation profile")]
    UnsupportedLiteral,
    /// Eval input is not one complete expression under the supported grammar.
    #[error("invalid dynamic expression syntax")]
    DynamicSyntax,
    /// Eval parsing exceeded its bounded syntax profile.
    #[error("dynamic expression syntax limit exceeded")]
    DynamicLimit,
    /// A dynamic grouping key contains a clock/random operation.
    #[error("dynamic grouping expression is not repeatable")]
    DynamicNotRepeatable,
    /// Clock-dependent evaluation requires a caller-issued clock provider.
    #[error("clock function requires a host clock")]
    ClockRequired,
    /// Rnd requires a caller-owned sequence instead of ambient process state.
    #[error("Rnd requires an explicit random sequence")]
    RandomStateRequired,
    /// CurrentUser needs caller-issued Access expression-session data.
    #[error("CurrentUser requires an explicit Access session")]
    AccessSessionRequired,
    /// The supplied session label is outside the bounded host-data profile.
    #[error("invalid Access session account name")]
    InvalidSessionName,
    /// Numbered Error lookup needs an explicitly selected localized catalog.
    #[error("Error requires an explicit message catalog")]
    ErrorCatalogRequired,
    /// A catalog resource violates the bounded flat-message contract.
    #[error("invalid error message catalog")]
    InvalidErrorCatalog,
    /// A selected provider failed; this is distinct from an undefined code.
    #[error("error message provider failed")]
    ErrorCatalogFailed,
    /// The host could not obtain a valid local clock reading or timezone.
    #[error("host clock or local timezone is unavailable")]
    ClockUnavailable,
    /// The host requested cancellation.
    #[error("expression evaluation cancelled")]
    Cancelled,
    /// Maximum expression nesting exceeded.
    #[error("expression depth limit exceeded")]
    DepthLimit,
    /// Consumable operation budget exhausted.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    /// Arithmetic operands are outside the operation's mathematical domain.
    #[error("invalid arithmetic domain")]
    Domain,
    /// A bounded financial iteration could not qualify a converged result.
    #[error("financial iteration did not converge")]
    Convergence,
    /// The operation does not accept these scalar kinds.
    #[error("expression type mismatch")]
    TypeMismatch,
    /// Expression-only data cannot pass the ordinary scalar result/storage boundary.
    #[error("expression-only value requires an explicit consumer")]
    NonScalarExpression,
    /// A numeric result cannot fit its carrier, or a floating quotient is 0/0.
    #[error("numeric overflow")]
    Overflow,
    /// A floating carrier contains NaN or infinity.
    #[error("non-finite number is not supported")]
    NonFinite,
    /// A divisor is zero.
    #[error("division by zero")]
    DivisionByZero,
    /// Implicit conversion would discard exact integer precision.
    #[error("numeric conversion would lose precision")]
    PrecisionLoss,
    /// Promotion/rounding for this pair is outside the implemented profile.
    #[error("numeric combination is not implemented")]
    UnsupportedNumericCombination,
    /// LIKE requires an explicit ANSI wildcard convention.
    #[error("LIKE requires a wildcard mode")]
    LikeModeRequired,
    /// An invalid wildcard list or descending range was supplied.
    #[error("invalid LIKE pattern")]
    InvalidPattern,
    /// Text casing requires a caller-selected language.
    #[error("text casing requires an explicit case locale")]
    CaseLocaleRequired,
    /// The case locale is not a supported language-identifier spelling.
    #[error("invalid case locale; expected a language identifier such as en-US or tr")]
    InvalidCaseLocale,
    /// String comparison needs an explicit collation implementation.
    #[error("text comparison requires a collation")]
    CollationRequired,
    /// A supplied comparison provider failed without exposing its private state.
    #[error("text comparison provider failed")]
    CollationFailed,
    /// Numeric Text conversion requires an explicit separator/currency profile.
    #[error("numeric text conversion requires an explicit number-format policy")]
    NumberFormatRequired,
    /// A numeric character code needs a host code-page policy beyond ASCII.
    #[error("character code requires an explicit code-page policy")]
    CharacterPolicyRequired,
    /// The selected character code page is outside the supported explicit profile.
    #[error("character code page is not supported")]
    UnsupportedCharacterCodePage,
    /// The selected code page has no valid character for the supplied byte.
    #[error("character byte is not mapped by the selected code page")]
    InvalidCharacterCode,
    /// Exact single-byte encoding cannot retain this character in the selected page.
    #[error("character is not representable in the selected code page")]
    UnmappableCharacter,
    /// A search provider returned a match outside its declared UTF-16 bounds.
    #[error("text search provider returned an invalid match")]
    InvalidTextMatch,
    /// A concrete conversion function cannot convert SQL NULL to its target type.
    #[error("invalid use of NULL in conversion")]
    InvalidNullConversion,
    /// An omitted date year requires a caller-issued reference year.
    #[error("date text with an omitted year requires a reference year")]
    ReferenceYearRequired,
    /// Date text conversion needs an explicit syntax profile.
    #[error("date text conversion requires an explicit date-format policy")]
    DateFormatRequired,
    /// System week settings require an explicit host calendar policy.
    #[error("date function requires an explicit calendar policy")]
    CalendarPolicyRequired,
    /// Date arithmetic requires a recognized Access interval spelling.
    #[error("invalid date interval")]
    InvalidDateInterval,
    /// A UTF-16 slice would contain an unpaired surrogate, outside the Text carrier.
    #[error("text result cannot be represented without losing UTF-16 data")]
    UnrepresentableText,
    /// A concatenated result exceeds the caller's byte ceiling.
    #[error("text output limit exceeded")]
    TextLimit,
    /// An expression array exceeds its explicit element ceiling.
    #[error("expression array limit exceeded")]
    ArrayLimit,
    /// An array bound or requested dimension cannot be represented.
    #[error("invalid array bounds or dimension")]
    ArrayBounds,
    /// Memory could not be reserved for a text result.
    #[error("text allocation failed")]
    Allocation,
}
