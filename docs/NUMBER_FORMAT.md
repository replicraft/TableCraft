# Explicit numeric Text conversion

CByte, CInt, CLng, CLngLng, CBool, CSng, CDbl, CCur and CDec now accept complete
numeric Text spellings under an explicit execution-local NumberFormat. Numeric
and Boolean input carriers retain their existing conversion behavior. NULL still
fails with InvalidNullConversion. Text conversion without a profile fails with
NumberFormatRequired; no database or OS locale is inferred.

| Profile | Decimal | Integer grouping | Optional currency token |
| --- | --- | --- | --- |
| invariant | period | None | None |
| en-us | period | comma, three-digit groups | `$` |
| de-de | comma | period, three-digit groups | `€` |

[CStr](CSTR.md) uses the selected decimal separator for numeric output, without
grouping/currency symbols; its date output has a separate DateTextFormat policy.
Text identity and Str/Val behavior are unchanged.

[IsNumeric](ISNUMERIC.md) also uses these profiles to recognize complete finite
numeric Text; malformed/overflowing text yields FALSE while operational failures
remain errors. Other scalar functions do not automatically gain Text coercion.

These are named syntax profiles, not complete emulations of Windows regional
settings. They are independent of collation, LIKE wildcards, character-code pages,
native field decoding, parameter transport, SQL literal syntax and Val.

```sh
tablecraft query eval "SELECT CDbl('\$1,234.50'), CInt('2.5')" --number-format en-us
tablecraft query run "SELECT CCur('1.234,50 €')" --number-format de-de
tablecraft query run "SELECT CLng(NBR_VALUE) FROM ROULETTE_TB" --database example.accdb --number-format invariant
```

When passing the first SQL example through a shell that expands dollar signs,
escape the dollar or use shell quoting appropriate to that environment.

## Complete-input grammar

The numeric body uses ASCII digits, one selected decimal separator, optional
integer grouping and an optional E/e/D/d exponent with a signed integer power.
Leading/trailing Unicode whitespace is accepted, as is whitespace around the
sign, currency token, separators and exponent marker/sign. Whitespace inside an
unseparated digit sequence fails: `1 2` is not 12. Grouped integers require one to
three digits in the initial group and exactly three in every subsequent group;
grouping is forbidden in fractional/exponent digits.

One leading ASCII sign can precede or follow a single leading currency token;
a trailing currency token is also allowed. Balanced outer parentheses indicate a
negative value and cannot be combined with another sign. Repeated signs/currency
tokens, malformed groups, multiple decimal separators, incomplete exponents and
trailing text fail. For example, en-us accepts `$-1,234.50` and `( $1,234.50 )`,
but rejects `1,23.0` and `12tail`. Unlike Val, this does not stop at a valid prefix.
Radix prefixes, type suffixes, localized Boolean words, alternate grouping widths,
additional regional currency/negative patterns and complete Variant coercion remain
unimplemented. Exact native whitespace/grouping/error precedence needs comparison.

## Target conversion and rounding

Integer targets round the exact decimal spelling halfway to even, then apply the
existing checked target range. There is no binary floating intermediate; Int64
extrema remain exact. Currency uses the same exact lexical rounding at scale four
and checks its signed-i64 coefficient. A value just below a midpoint is not first
rounded to Decimal scale 28 or binary Double and then rounded again.

CDec chooses a scale within 0..28 and the 96-bit magnitude envelope, retaining the
written scale when it fits. It rounds the original spelling directly; if the
29-digit top band exceeds the coefficient envelope, it reduces scale and rounds
the original input again. It never rounds an already rounded candidate. Out-of-
range integral magnitudes fail. This is an explicit initial text-conversion
policy; native CDec precision, scale retention and range/rounding order remain
unqualified. Native field precision constraints still apply separately.

CSng/CDbl normalize separators and parse directly into the requested IEEE width.
Finite underflow, including signed zero, is permitted; overflow fails. CSng does
not parse through Double. CBool recognizes trimmed case-insensitive True/False and
case-sensitive #TRUE#/#FALSE#; other valid numeric Text converts through Double
and tests zero/nonzero. Consequently a tiny numeric Text can underflow to FALSE,
while an already materialized nonzero Decimal remains TRUE under existing rules.
NULL, arity, qualifier and eager-error behavior remain unchanged.

The selected rust_decimal kernel still owns bounded numeric/IEEE conversion and
decimal arithmetic behind TableCraft carriers. Arbitrary-precision input text
requires lexical guard/sticky-digit rounding before carrier construction: parsing
through a bounded Decimal first can lose which side of a later midpoint the input
occupied. This reuses the existing exact Val suffix scanner; it does not add another
decimal arithmetic dependency or change core value constructors/equality/Serde.
The shared scanner now classifies huge exponent overflow explicitly, without
expanding exponent-sized buffers. Val retains its separate grammar and behavior.

## Hosts, bounds and evidence

Library callers select Evaluator::with_number_format or ExecutionOptions.number_format;
Conversion::apply_text accepts a concrete policy and initially empty NumericBuffer.
Conversion::apply accepts numeric/Boolean and [legacy DATE serials](DATE_NUMERIC.md)
without Text parsing. Prepared plans do
not capture a format; nested queries and each optional SQLite worker request use
the current execution's setting. SQLite affinity is unchanged and no conversion
is pushed into native storage or SQLite. The CLI flag is available only on query
eval/run; parsing and inert inspection never execute it.

Whole-input and normalization scans charge work and check cancellation. Normalized
UTF-8 bytes are charged before fallible allocation under existing input/value and
operation ceilings. Partial buffers/results are discarded and failed CLI commands
publish no partial stdout. Host callbacks/budgets remain explicit. There is no new
dependency, fixture or GUI/network runtime requirement.

Focused tests cover all profiles, grouping/currency/exponents, exact range and
midpoint cases, direct Single parsing, Decimal precision, cancellation/allocation
limits, parameter reuse, grouping, separate nested-query cases, CLI transport and
worker isolation. The [independent recipe](testing/compare_number_format.py) uses
Python Decimal/integer models and optional sequential/indexed projections over
Jackcess-decoded inputs, checking the source hash after reads. These are independent
models/readers; native Windows Access comparisons remain unrun. All nine functions
remain Partial in the [built-in tracker](ACCESS_BUILTINS.md).

Primary references: Microsoft's [conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
cover regional settings and target/rounding behavior; [MS-VBAL String coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
describes whole-string regional numeric input, exponents and Boolean spellings.
These references guide the explicit profile above; they do not establish complete
regional or native conformance.

The same execution-local policy now governs Text condition/index slots in
[IIf/Switch/Choose](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states).
Branch/choice payloads are retained unchanged. Missing policy fails explicitly;
no locale or collation is inferred from a Text condition.

## Logical Text conversion and predicates

[Logical operators](INTEGER_LOGIC.md#rounded-numeric-and-text-operands) now use
this execution-local policy for numeric Text operands, with checked half-even
Long or Int64-selected LongLong targets. Unary Not uses Long. Parsing/rounding
retains exact large Text integer digits and cumulative normalization limits.
Read-only WHERE/HAVING/ON and collection member predicates use CBool instead,
retaining fractional zero/nonzero truth and the supported Boolean words. No
locale/collation is inferred, pure helpers still reject Text, and UPDATE's final
predicate remains its separately qualified Boolean/NULL boundary. Native Access
regional, Variant and subtype qualification remains open.

[SQL slicing counts](SLICE_COUNTS.md) now use the same explicit profile for Left,
Right and both Mid positions. They normalize directly into checked Long results;
normalization and final output share the operation budget without cloning the
source. Pure text kernels and other argument slots retain their own contracts.

Space/String count slots and InStr/InStrRev written start slots now also use
[contextual repetition/search conversion](TEXT_CONSTRUCTION.md#contextual-repetition-counts).
String's Text character slot remains literal; comparison modes and search source
operands retain their own contracts. NULL shortcuts and validation order remain
function-specific rather than a blanket numeric Text preprocessing pass.

Replace's optional start/count slots now also use [contextual conversion](TEXT_REPLACEMENT.md#contextual-start-and-count-conversion).
Both normalizations and final output share budgets while the recipe borrows the
original strings. The comparison slot and source carriers retain their own
contracts; NULL/default/error precedence remains function-specific.

[Comparison modes](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
and [character/raw-byte numeric slots](CHARACTER_FUNCTIONS.md#contextual-character-code-conversion)
now also use this explicit policy. Modes, codes and positions normalize without
reinterpreting source strings or guessing a code page. Per-function validation
order and provider/range/surrogate constraints remain authoritative. StrConv's
separate flags/LCID conversion profile is unchanged.
