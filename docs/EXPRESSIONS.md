# Scalar expression evaluation

`crates/query/tablecraft-expressions` owns evaluated scalar rules over core
`Value` carriers. Its default build and direct scalar APIs are independent of SQL
parsing, database connections, GUI hosts and async runtimes. The optional `sql`
feature adds an evaluator of TableCraft-owned expression syntax. The syntax parser remains in
`tablecraft-sql`; [tablecraft-query](QUERY_EXECUTION.md) now supplies separate
binding, query plans and bounded row execution.

This is an initial, explicit scalar profile, not complete Jet/ACE or VBA Variant
semantics. Core values describe materialized carriers, not the declared/Variant
type metadata required to infer every native promotion. Documentation-derived
tests are not observed native Access results.

## Implemented rules

| Area | Implemented behavior |
| --- | --- |
| Truth | TRUE/FALSE/unknown, Boolean/NULL conversion, NOT/AND/OR/XOR/EQV/IMP; predicate selection accepts only TRUE |
| Comparison | Either NULL returns unknown, including NULL = NULL; all six comparison operators use numeric ordering, not `Value::Eq` |
| Exact numerics | Boolean TRUE becomes -1 and FALSE becomes 0 in the numeric domain; Byte/Int16/Int32/Int64, Currency and Decimal compare without floating conversion or scale-alignment overflow |
| Floating comparison | Finite numeric comparison, signed zeros equal; Double rounds to Single precision when those two carriers are compared; overflow/non-finite values rejected |
| Integral coercion | Explicit Byte/Int16/Int32/Int64 targets; fractions round halves to even; bounds checked after rounding; NULL remains NULL |
| Integral arithmetic | Checked +, -, *, unary minus; results use Int64; overflow errors instead of wrapping or guessing Variant widening |
| Floating arithmetic | +, -, *, / and ^ over supported integers/floats; finite Double results; division by zero and overflow are errors |
| Integer division/remainder | `\` and Mod round numeric operands halves to even, then use a quotient truncated toward zero and a dividend-signed remainder; Int64 result |
| Currency arithmetic | Exact Currency +/- Currency/integer and Currency * integer, retaining scale four and rejecting overflow |
| LIKE | Explicit ANSI-89/ANSI-92 mode, host scalar collation, lists/ranges and three-valued NOT; bounded iterative matching over Unicode scalar units ([contract](PATTERN_MATCHING.md)) |
| Concatenation | Text/NULL `&` treats one NULL as empty and retains NULL for two NULL operands; text `+` propagates either NULL; UTF-8 output byte ceiling checked before allocation |

Decimal scale and core carrier equality remain unchanged. Exact numeric comparison
compares signs, whole parts and fractional digits without rescaling a coefficient
past its range. Large integer digits are never silently rounded into a Double:
unsupported inexact integer-to-float conversion returns `PrecisionLoss`.

Text comparison requires a caller-supplied `Collation`; no locale, Unicode
normalization, case-folding or Rust string order is silently selected. A host can
supply a deterministic test collation without claiming it matches Access.

`EvalError` uses thiserror and carries no value, SQL or field-name payloads.
Scalar routines accept `Value`, so Empty/Missing/Error expression-only states
cannot silently enter them as NULL. Omitted function arguments retain their
separate syntax representation.

`Evaluator::apply_unary` and `apply_binary` also accept already evaluated operands
for the bound query executor. The caller charges node/row work; evaluator Text
intermediates continue consuming the supplied byte budget.

## Explicit limits of this profile

Arithmetic result carriers are TableCraft's initial checked Int64/Double profile,
not VBA's automatic Byte/Integer/Long/Single/Variant promotion ladder. The
nullable `coerce_integer` kernel is not an implementation of the CInt/CLng/CByte
functions: it propagates SQL NULL and checks the signed -1 numeric Boolean
representation against the chosen target range. Explicit
coercion can select a narrower integral result. Decimal binary arithmetic and
Currency division, Currency * Currency, and Currency/Decimal mixed with floats
are rejected until rounding and the larger Variant Decimal envelope have their
own implementation and evidence. Decimal/Currency can still participate in exact
comparison and explicit integral coercion (including rounded `\`/Mod operands).

Logical operators accept Boolean/NULL only; numeric bitwise operators and implicit
numeric-to-truth conversion are not implemented. Numeric text parsing, mixed
text/numeric `+`, numeric/date string rendering, GUID ordering/binary comparisons,
locale-dependent conversions, built-ins beyond IsNull/IIf and complete VBA expression
states are not implemented. Unsupported combinations fail explicitly.

The direct text APIs require a byte ceiling. They reserve fallibly after checking
the combined length. Other scalar functions perform bounded work on fixed-size
carriers; a caller supplying a collation owns that callback's work and semantics.

## Bounded expression trees (`sql` feature)

Enable `tablecraft-expressions`' `sql` feature to use `evaluation::Evaluator`.
This consumes `tablecraft_sql::select::Expression`, with no upstream AST types.
It evaluates scalar nodes, not SELECT statements or rows from a database.
The CLI's `query eval` command now invokes it for one source-free SELECT's scalar
projections, with `NoBindings` and no implicit collation. It buffers a header and
one result row, rejecting database sources, parameters and query clauses that
would need execution. `query parse` remains syntax-only. See
[CLI behavior](DEVELOPMENT.md#cli-behavior) for output policy, limits and errors.

Supported nodes are NULL, Boolean, signed i64 literals, Text, finite fractional
or exponent literals, host-bound field/parameter references, unary numeric signs
and NOT, supported binary operators, IS NULL/IS NOT NULL, BETWEEN/NOT BETWEEN
and IN/NOT IN lists. Integer literals
materialize as Int64; fractional/exponent literals materialize as Double, not
exact Decimal. Numeric literal type inference for native Jet/ACE is not claimed.
Unmaterializable bare integer/hex/octal spelling outside the i64 syntax carrier,
non-finite float literals, underflow of a nonzero literal to zero and literals
longer than 128 bytes fail explicitly. Ordinary floating literal rounding to
binary64 remains distinct from exact Decimal value construction.

The host implements `Bindings` to provide borrowed core values for written field
references, including qualifiers and parameters. `NoBindings` supports constants
and rejects unresolved names. The evaluator has no filesystem, connection,
catalog, clock, external function or identity-state access. Binary materialization,
unrecognized/qualified calls,
@@IDENTITY, members and subqueries are explicit
unsupported cases in this first tree evaluator, even though the parser owns those
syntax forms. Omitted function arguments are never evaluated as stored NULL.

Both binary operands are evaluated left-to-right. Boolean truth and NULL do not
hide evaluation errors in another operand. This is an explicit scalar evaluation
order, not a promise about native query optimizer order or VBA short-circuiting.

Each evaluator borrows a caller-owned `ResourceBudget`. It charges one step per
visited node and cumulative Text bytes before allocating literal copies, binding
copies or concatenated results. Failed evaluations keep work already charged;
reusing the evaluator shares the remaining budget. Rows are not charged. String
capacity is reserved fallibly. This is an allocation/work allowance, not exact
resident memory accounting or a deadline.

`EvalLimits` clamps maximum depth to 32 nodes and each Text value to 65,536 UTF-8
bytes, even for manually constructed trees; callers can lower either ceiling.
An optional host cancellation callback runs before every node, before bindings.
Hosts own the boundedness and side effects of resolver/collation/cancellation
callbacks; the evaluator cannot interrupt a callback while it runs.

## References and verification

The authored tests follow Microsoft primary references; they do not copy fixture
SQL or execute content from the supplied ACCDB.

| Rule | Reference |
| --- | --- |
| Access expression/operator scope | [Access operators](https://support.microsoft.com/en-us/access/table-of-operators) |
| Boolean/NULL tables | [And](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/and-operator), [Or](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/or-operator), [Not](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/not-operator), [Xor](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/xor-operator), [Eqv](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/eqv-operator), [Imp](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/imp-operator) |
| Numeric comparison and mixed precision | [Comparison operators](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/comparison-operators) |
| Boolean numeric conversion | [Boolean data type](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/boolean-data-type) |
| Coercion, rounding, locale and Variant Decimal boundaries | [Conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions) |
| Arithmetic and native promotion distinctions | [Addition](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator), [subtraction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minus-operator), [division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/forwardslash-operator), [integer division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/backslash-operator), [Mod](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mod-operator) |
| Distinct concatenation behavior | [Ampersand](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ampersand-operator), [Access text concatenation](https://support.microsoft.com/en-us/access/combine-text-values-by-using-an-expression) |

Local checks on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026 pass
formatting and Clippy with warnings denied in both feature configurations.
The default profile has 12 tests and four fixed-seed properties; enabling `sql`
adds nine tests (including a fifth property) and an API doctest. Each property
uses 256 cases. Tests cover complete Boolean/NULL
tables, exact comparisons beyond f64 precision, half-even rounding, integer
extremes, signed quotient/remainder identities, Currency boundaries and Unicode
byte limits. Native Access comparison and other platforms remain unverified.
Parsed-expression tests also cover precedence, host binding, eager errors,
unsupported syntax, callback collation, cancellation, cumulative byte/step
charges and lowered/clamped depth/output limits.

Legacy Date/Time now supports temporal comparison and bounded ISO literals under
the SQL feature. [Native date contracts](NATIVE_TABLES.md#legacy-datetime) describe
negative-day semantics, rounding boundaries and rejected locale/timezone coercions.

GUID literals and equality/inequality are supported via canonical bytes; text/
numeric coercion and GUID ordering remain unsupported when evaluated.

## Date/Time Extended comparison

Two extended-date carriers compare by civil date and time at exact 100ns
precision. NULL still yields unknown. Mixed legacy/extended dates return
`TypeMismatch`; this profile does not invent implicit conversion or timezone
semantics. Query filtering, ordering, grouping and distinct reuse this comparison.
[Native read evidence](NATIVE_ACE16.md#modern-scalar-reads) remains independent-reader
evidence, not Microsoft Access query conformance.

## Shared range and membership predicates

The SQL-independent `predicates` module supplies eager inclusive BETWEEN with
either written bound order and a constant-space IN-list truth accumulator. Hosts
supply comparison/collation, typed error and work/cancellation policy. Query
execution and the source-free scalar tree evaluator now use these same kernels;
no list of materialized Values is needed. IN retains exact numeric comparison,
NULL/unknown and NOT semantics. A prior match does not suppress later errors.
An empty syntactic IN list remains invalid; an empty subquery is a separate case.

The tree evaluator charges candidates and comparisons to its shared budget and
checks cancellation; arbitrary calls/subqueries/host access remain unsupported.
LIKE uses the separate [bounded pattern kernel](PATTERN_MATCHING.md) with explicit
mode and collation. Native validation-rule preflight still rejects LIKE.
Text comparison still needs explicit collation. Native numeric field/table audits
admit these predicates only after their existing pure-expression preflight; text
rules and unresolved NULL validation results stay unqualified.

Fixed-seed interval checks cover full i64 values without float conversion, along
with reversed bounds, unknown truth, eager errors, cancellation and budget limits.
Original [native rule fixtures](testing/PredicateRuleFixture.java) separately check
independent writer acceptance/rejection on both qualified newer families. This
is not Microsoft Access conformance or a new writable ACE profile.


## Explicit IsNull and IIf built-ins

The parser-independent `functions` module owns a closed, allocation-free built-in
lookup and value kernels. `IsNull(value)` returns Boolean true only for stored
NULL; zero, empty text/binary and other materialized values are distinct. Core
Empty/Missing/Error expression states are not coerced into stored values.
`IIf(condition, truepart, falsepart)` selects and moves one already evaluated branch
without result-carrier promotion, cloning or allocation. Both scalar syntax and
bound retrieval evaluate condition, truepart and falsepart in written order.
An error or resource failure in the unselected branch still fails the expression.
No optimizer path drops that branch or its required source fields.

Supported conditions are Boolean, exact integer/Currency/Decimal and finite Single/
Double: zero is false and nonzero is true. Exact coefficients are tested without
floating conversion, so Int64 and 28-digit Decimal precision remain intact. NULL
selects falsepart. Text/date/GUID/binary truth conversion remains unsupported;
there is no implicit locale-sensitive text parsing. Other logical operators retain
their existing Boolean/NULL profile. Branches retain exact materialized values;
result-column type inference and complete native Variant promotion remain future
work. The standalone syntax evaluator retains its existing Binary-binding limit;
query retrieval can carry bounded Binary values through these functions.

Unqualified ASCII-insensitive IsNull(one expression), IIf(three expressions)
and Nz(one or two expressions) resolve. Unknown functions, module qualifiers, wildcards, omitted positions and
wrong arity fail explicitly. These are pure Rust built-ins, not VBA execution or
an external function registry. UPDATE binding retains its prior no-call contract.

```sh
tablecraft query eval "SELECT IsNull(Null), IsNull(''), IIf(Null,1,2)"
tablecraft query run "SELECT ID, IIf(IsNull(Label),'missing',Label) AS DisplayLabel FROM Items" --database copy.accdb
```

The contracts draw on Microsoft's [IsNull documentation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isnull-function),
[IIf argument/eager-evaluation documentation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/iif-function),
[numeric Boolean conversion](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
and [Access expression examples](https://support.microsoft.com/en-us/access/examples-of-expressions).
These are documentation-derived semantics, not observed native Access optimizer,
coercion or result-type behavior. Windows comparison remains unrun.

Focused tests cover full-width numeric truth, nullable/empty distinctions, argument
order, unselected failures/allocations, cancellation, grouped/correlated retrieval,
projection pruning, saved-query execution and buffered CLI errors. Independent
original Jet 4/ACE 2007/2016/2019 getters compare individual null tests and conditional
wrappers across all fourteen stored scalar carriers, plus Currency/Decimal/Single
conditions; exact source bytes remain unchanged. No native SQL is executed by that
oracle and no registry dependency is added.


## Query-context Nz

`Nz(value[, fallback])` is a pure shared kernel used by scalar SELECT evaluation,
retrieval and qualified saved queries. Non-NULL input passes through with its exact
carrier. NULL selects an explicit fallback unchanged, including explicit NULL.
With no fallback, NULL becomes zero-length Text, following Microsoft's
[query-expression documentation](https://support.microsoft.com/en-au/access/functions/nz-function).
This query behavior does not introduce VBA's contextual Empty/zero semantics.

Supplied operands evaluate in written order under the existing eager scalar-call
profile; errors, cancellation and allocations in an unused fallback remain visible.
Selection moves a value without allocation, coercion or carrier promotion. Empty
text for an omitted fallback needs no heap allocation. All provided fields remain
in projection and correlation mapping, and aggregate/group rules apply to both
operands. `Nz(Null,)`, qualified calls, wildcards and wrong arities fail explicitly.
UPDATE retains its existing no-call boundary. Native Access optimizer, coercion
and evaluation-order comparisons remain unrun; these are explicit implemented
contracts rather than a claim of full Access expression equivalence.
