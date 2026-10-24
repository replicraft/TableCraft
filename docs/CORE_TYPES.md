# Implemented core types

`crates/foundation/tablecraft-core` provides in-memory values, scoped identities, validation
errors and operation budgets. It uses `thiserror`, with optional `serde` support
and development-only `proptest`/`serde_json`. It does not read database files or
define a production persisted or wire format. The CLI consumes these values for
supported scalar evaluation and read-only native queries.

Jiff civil types now supply checked calendar validation/conversion. Core still
owns exact Automation DATE bits, field year bounds and extended 100 ns precision;
optional Serde retains those representations. The unpublished from_calendar and
calendar_rounded_to_nanos APIs accept/return jiff::civil::DateTime. No timezone is
attached implicitly. [Dependency records](DEPENDENCIES.md) and
[native benchmarks](testing/NATIVE_BENCHMARKS.md) document the migration and costs.

Legacy calendar projection rounds to nanoseconds with ties to even. It folds a
possible midnight carry into one checked civil-day addition, then combines the
date with a separately checked Jiff time. Calendar encoding counts integral
nanoseconds within the already validated civil time; this fits exactly in both
i64 and f64 before division into a day fraction. Stored bits, canonical negative
DATE encoding and checked overflow behavior are unchanged. Calendar conversion is
explicit: stored-date comparisons use day/fraction ordering without constructing
a civil calendar. Golden boundary/rounding cases and fixed-seed properties compare
the specialized adapter against Jiff's general duration arithmetic.

The source review below was performed on 17 October 2026. Microsoft documents
the external constraints; the Rust carriers and boundaries are TableCraft design
choices. Passing these tests establishes local invariants, not native Access
compatibility.

## Source layout

Code is organized by feature, with focused unit tests alongside each feature:

```text
src/
  ids/
    identity.rs       Semantic identity domains and source generations
    record.rs         Scoped backend record locators
    cursor.rs         Scoped cursor bookmarks
  values/
    currency.rs       Fixed-scale money and checked arithmetic
    decimal.rs        Exact decimal validation, display and rescaling
    floating_point.rs Single/Double bit carriers
    guid.rs           Canonical GUID values
    datetime/
      legacy.rs       Automation DATE validation and representation
      extended.rs     Gregorian extended date/time validation
    stored.rs         Storage value composition and redacted Debug
    expression.rs     Language states and storage conversion
    expression_string.rs Lossless owned String byte ranges
    expression_string/   Focused byte-range tests
    validation.rs     Shared value-validation errors and state categories
    fixed.rs          Private formatting shared by Currency and Decimal
  limits.rs           Resource accounting, errors and focused tests
```

Each directory's `mod.rs` declares its features and re-exports the existing
`ids::*` and `values::*` API. Feature module paths are also available for explicit
imports. `tests/core_contracts.rs` retains public-API tests that cross feature
boundaries: locator ownership, storage/expression separation and redaction.

## Scalar representation and sources

| Implemented carrier | Decision | Primary reference |
| --- | --- | --- |
| `Byte`, `Int16`, `Int32`, `Int64` value variants | Retain unsigned 8-bit and signed 16/32/64-bit ranges separately. Availability of Large Number is a backend capability, not implied by an `i64` carrier. | [Access SQL data types](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types), [DAO DataTypeEnum](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/datatypeenum-enumeration-dao) |
| `Single`, `Double` | Keep IEEE bit patterns, including signed zero and NaN payloads, with no numeric promotion. This preserves a carrier; it does not assert that Access accepts every bit pattern. | [VBA data type summary](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/data-type-summary) |
| `Currency` | Signed `i64` in ten-thousandths. Checked addition/subtraction returns an overflow error. Display is locale-independent with four fractional digits and no assumed currency symbol. | [Currency data type](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/currency-data-type) |
| `Decimal` | Signed `i128` coefficient bounded by a 96-bit magnitude, with retained scale 0..=28. The 29-digit top band is bounded by 2^96-1; Access fields retain separate precision 1..=28 constraints. | [Access SQL DECIMAL precision and scale](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types) |
| `Null`, `Text`, `Binary`, `Boolean` | Keep SQL NULL, zero-length text and zero-length binary distinct. Text is decoded Unicode; binary has no implicit text or OLE interpretation. Boolean is logical and has no implicit numeric coercion. | [Access field types and properties](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties) |
| `Guid` | Preserve a canonical 16-byte value in textual group order; native field decoding must handle native byte order explicitly. | [Microsoft GUID structure](https://learn.microsoft.com/en-us/windows/win32/api/guiddef/ns-guiddef-guid) |
| `LegacyDateTime` | Retain validated Automation DATE double bits and the year-100-through-9999 day envelope. The whole day truncates toward zero; the fractional time is absolute, including before the epoch. No timezone or float ordering is imposed. | [DATE type](https://learn.microsoft.com/en-us/cpp/atl-mfc-shared/date-type?view=msvc-170), [VBA Date range](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/data-type-summary) |
| `ExtendedDateTime` | Validate Gregorian components for years 1..=9999, with seven fractional-second digits in 100 ns units. No timezone is attached. | [Access Date/Time Extended](https://support.microsoft.com/en-us/access/using-the-date-time-extended-data-type), [SQL Server datetime2](https://learn.microsoft.com/en-us/sql/t-sql/data-types/datetime2-transact-sql) |

`Decimal::new` now covers the full signed 96-bit magnitude documented by
[Microsoft's VBA Decimal reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/decimal-data-type),
with scale 0..=28. MAX_COEFFICIENT is 79,228,162,514,264,337,593,543,950,335;
MAX_PRECISION reports its 29 coefficient digits, not an unrestricted 29-digit
range. Native fields remain precision 1..=28 through
[shared schema constraints](SCHEMA.md#declared-decimal-field-constraints) and native
codec checks. A valid scalar is not permission to store it in a column.

The carrier provides no float conversion, implicit rounding or general decimal
arithmetic. `rescale_exact` rejects nonzero discarded digits and coefficient
overflow. Trailing zeroes remain representational; negative zero/original native
bytes require separate preservation. Optional Serde retains the same
coefficient/scale wire shape and validates through the widened constructor,
including rejecting magnitude 2^96 and scale 29. Previously valid encodings remain
valid; some 29-digit coefficients previously rejected are now valid scalars but
still invalid field assignments. This is an intentional scalar validity expansion.

The Access Date/Time Extended article describes seven fractional digits and
datetime2 compatibility, but also labels accuracy as one nanosecond. This
implementation uses the seven-digit envelope, consistent with the datetime2
reference's 100 ns units. It does not claim to implement or qualify Access's
42-byte native encoding. Legacy DATE retains its original double rather than
quantizing it to milliseconds or converting it to an extended timestamp.

`Value` equality is **representation equality**. For example, decimals `1.0` and
`1.00`, positive/negative float zero, and the two legacy DATE encodings of an
epoch-day time can differ. SQL equality, collation, NULL propagation and temporal
ordering must be implemented separately. No ordering trait is supplied for values.

## Language-only states

`ExpressionValue` wraps a stored scalar or the distinct `Empty`, `Missing` and
`Error` states. `TryFrom<ExpressionValue> for Value` rejects all three language-only
states with typed `ValueError::ExpressionNotStorable`; none becomes NULL or an
empty string. An error code is an opaque evaluator-supplied `u32`. The carrier itself does not
perform CVErr conversion or exception handling.

The expression library now constructs standard Error codes through
[CVErr](VARIANT_ERRORS.md), preserves Error data through explicit selectors and
exposes an expression-result evaluation API. Stored Value and its conversion
refusals are unchanged. The query expression-result API now carries Error cells
through derived rows and results; its scalar-only API retains checked publication.
Neither API admits Error into source snapshots or database writes.

[Type/state inspection](VALUE_INSPECTION.md) now consumes these explicit states
without converting them to storage. Bounded array intermediates live in
`tablecraft-expressions::arrays`; neither stored Value nor ExpressionValue has
acquired an array variant or an implicit array-to-scalar conversion.

This separation follows Microsoft's distinction between uninitialized `Empty`
and `Null` in [Variant data type](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/variant-data-type),
and omitted optional arguments in [IsMissing](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ismissing-function).
It does not implement VBA coercion, objects/Nothing, arrays or evaluation.

## Lossless expression String buffers

`ExpressionString` owns an adopted byte buffer and checked active range. The
existing `tablecraft-expressions` ByteString now uses it as its backing value;
this is a working shared representation, not a new stored Value variant.
Construction moves the caller's already charged allocation without copying or
decoding. Consuming `select` validates an exact range relative to the active
bytes; reversed/out-of-range requests return `ExpressionStringRangeError`.
Nested selections retain the same allocation and do not realign byte boundaries.
Odd byte lengths and unpaired UTF-16 units are preserved. Debug reports length
without payloads.

Core owns no Unicode publication, formatting, SQL truth/coercion or function
position rules. The expression consumer retains its existing signed Long byte
limit, one-based/clamped slicing and budgeted strict UTF-8 publication. No new
Serde representation or cloning API is introduced. ExpressionValue, stored Text,
source snapshots and write admission remain unchanged; raw relational cells are
still unfinished. This shared ownership boundary precedes that integration.

Four focused controls include retained-allocation/nested-range goldens, invalid
and empty ranges, redaction and a fixed-seed 256-case slice-content model. Existing
byte-function, query and independent CLI controls verify the consuming wrapper.

## Identity and lifetime

`ObjectId`, `TableId`, `OperationId` and `CursorId` are distinct opaque `u128`
newtypes. They do not generate UUIDs, assert uniqueness, identify native Access
catalog objects, or add fields to existing tables. Owners must allocate tokens
and maintain any durable mappings. Explicit integer access is available for
mapping/encoding; implicit conversion between identity domains is absent.

`RecordId` includes a `TableId`, `SourceGeneration` and backend locator bytes.
`CursorBookmark` instead includes a `CursorId`, generation and bookmark bytes.
The owner must issue a new generation when its tokens become invalid and must
check scope at use. Locators can be native keys or other supported backend tokens;
the core does not interpret or validate their contents. `SourceGeneration` is
neither a semantic revision nor a transaction isolation guarantee.

This scope is a TableCraft contract informed by Microsoft's [DAO Bookmark
reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset-bookmark-property-dao):
bookmarks are not record numbers and independent recordsets cannot interchange
them. The API does not turn a bookmark into a durable mutation key or promise a
specific engine's lifetime rules.

## Errors, diagnostic output and bounds

`ValueError` and `LimitError` implement `std::error::Error` with typed variants or
fields, using `thiserror` derives. Validation messages contain no rejected cell values. `Value` and
`ExpressionValue` debug output redacts payloads; locator debug output redacts
tokens. Direct numeric carrier formatting deliberately exposes its value, so
callers must not treat every `Debug`/`Display` implementation as safe logging.

`ResourceBudget` has explicit remaining byte, row and work-step limits. Charges
are checked before counters change. A failed charge leaves the allowance intact;
counted allocations also check multiplication overflow. There are no implicit
unlimited defaults, automatic resets or clone-based budget duplication. These are
TableCraft operational limits rather than Microsoft format limits. Callers must
charge before allocation/work; the type does not intercept allocations, enforce
deadlines, cancel tasks, or define what an operation counts as a step.

## Optional Serde support

Enable `tablecraft-core/serde` to serialize scalar carriers and `Value` for
value exchange and round-trip checks. Serde is independent of the output format.
The core keeps integer representations: float bits/legacy DATE bits use unsigned
integers, Currency uses its scaled `i64`, and Decimal uses its `i128` coefficient
plus retained scale. GUIDs use canonical byte arrays. `Value` uses named
type/content tags. No decimal or integer is routed through a floating-point conversion.

Decimal, legacy DATE and extended dates deserialize through their validating
constructors using [Serde's `try_from` support](https://serde.rs/container-attrs.html#try_from).
IDs, source generations, expression states, record locators, bookmarks, errors
and consumable resource budgets are not serialized. Add serde to another concept
only when a concrete consumer needs it.

The chosen serializer must support these integer widths. `serde_json` is only a
development dependency used for direct, typed round trips; consumers such as
JavaScript, or an intermediate generic JSON value, may impose narrower numeric
limits. A production JSON adapter can choose strings for such consumers without
changing the core's Serde representations. These impls are not a versioned
external protocol or native database codec. Input size and allocation limits must
be enforced by the caller before decoding; Serde does not enforce ResourceBudget.
Serialization exports actual values and is not redacted diagnostic output.

## Verification and current boundaries

Feature unit tests and public-API integration tests cover coefficient/range extremes, every decimal scale,
exact rescaling over a deterministic coefficient/scale sweep, Currency overflow,
negative legacy dates, Gregorian leap years, invalid calendar/time components,
100 ns fractions, NULL/empty/expression separation, GUID and float bit retention,
locator scope, redaction and budget overflow. Compile-fail doctests verify that
distinct ID domains and bookmarks cannot substitute for mutation keys.

Proptest additionally exercises Currency arithmetic, Decimal acceptance/rescaling,
legacy DATE validation and budget multiplication against independent wider-integer
checks where applicable. Properties run 256 cases with a fixed seed and keep
failure persistence enabled. Serde tests cover every scalar variant, invalid
constructor inputs, full-width integers, Unicode, decimal scale, arbitrary
float bits and signed zero. Generated JSON round trips use
typed deserialization rather than a generic JSON value for large coefficients.

Core carriers do not themselves establish native codec fidelity, on-disk round
trips, production serialization, AutoNumber behavior, collation, long-value or
complex-field support. Working schema/query/native consumers have their own
qualification records. Explicit checked calendar conversion and legacy Date/Time
reading are documented in [NATIVE_TABLES.md](NATIVE_TABLES.md#legacy-datetime).
