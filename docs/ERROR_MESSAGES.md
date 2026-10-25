# Numbered error-message expressions

`Error(number)` and `Error$(number)` return Text describing an explicit signed
Long message number. They neither raise an error nor construct Variant Error data. For
example, the CLI's selected English catalog returns `Division by zero` for
`SELECT Error(11)`. `Error(0)` returns empty Text without consulting a catalog.
Omitted-number `Error()` depends on the latest VBA runtime error and remains
unimplemented with the deferred `vba-rs` runtime. TableCraft's typed operational
failures never become that state. See [Variant Error values](VARIANT_ERRORS.md)
for the separate CVErr constructor and explicit numeric conversion.

## Execution contract

The explicit argument uses checked Long conversion, including half-even numeric
rounding, followed by the numbered-message range check: all negative Long values
and 0..65535 are admitted. Positive numbers above 65535 report Overflow. This is
separate from CVErr's unsigned standard-code range. NULL, invalid types and Long
conversion overflow fail. Numeric Text needs the
existing explicit number-format policy. Error data needs an explicit numeric
conversion; it is not silently interpreted as a code. Argument evaluation remains
eager, and unsupported arity is rejected before operands are evaluated.

Nonzero lookup requires caller-issued `ErrorMessages` in scalar Evaluator or
query ExecutionOptions. The provider must be immutable, deterministic and bounded
for one execution, with no external effects. Its `message` argument is the checked
`ErrorMessageCode`; `number()` returns the exact signed i32. Host implementations
must adopt this typed signature instead of u16. The provider returns a borrowed message and
owns the localized fallback for valid undefined codes. Missing context and a
provider failure are typed errors; a failure never silently becomes the unknown
message. The evaluator checks cancellation/work before and after lookup and
charges the returned UTF-8 Text before allocating it. Hosts own provider setup
bounds; a provider callback cannot be forcibly interrupted.

Plans capture neither catalog nor language. Nested queries, domain expressions,
closed Eval strings and repeatable grouping keys share the execution's provider.
The SQLite worker accepts an owned Arc provider for each request and supplies no
ambient default. The CLI explicitly selects the lazy English provider for both
`query eval` and `query run`, including file-backed runs. All output remains
buffered before publication. No OS locale, database language or UI language is
inferred, and no `sys-locale` dependency is needed for this explicit choice.

## Composition metadata

Successful `Error(number)` and `Error$(number)` results now declare nonnullable
Text during metadata-only binding. No message provider is consulted, and a NULL
or invalid argument still fails at execution rather than producing a nullable
message. Named/derived queries and First/Last inherit that descriptor under their
existing NULL rules.

This fixes an admitted composition failure: `SELECT Error$(0) UNION SELECT 1`
previously kept Dynamic metadata and refused mixed Text/integer duplicate
comparison. It now uses the [existing UNION common-Text conversion](UNION_TYPES.md)
with explicit numeric formatting, yielding empty Text and Text `1`. Conversion
precedes deduplication, so a provider message `1` and numeric `1` collapse under
an explicit equal-text comparer. Missing catalog/format policy, provider failures
and later arm errors remain errors with no complete result. The descriptor is a
current evaluator guarantee, not native Access expression-type qualification.

## Optional Fluent catalog

The `tablecraft-expressions/fluent-catalog` feature implements ErrorMessages using
fluent-rs (`fluent-bundle`, `fluent-syntax`) and explicit `unic-langid` identifiers.
The feature is enabled by the CLI; minimal expression consumers need not include
it. Only flat Text messages named `error-N` for canonical positive codes,
`error-negative-N` for canonical negative magnitudes and a required `error-unknown`
fallback are accepted. Zero, leading zeros, signed-positive spellings and out-of-
range keys are rejected. For example, `error-negative-2147483648` retains Long MIN;
it is not an unsigned reinterpretation or a mapping to an HRESULT description. Duplicate IDs, attributes, terms,
references, placeables, variables and formatter functions fail validation.
Resources are capped at 128 KiB and language identifiers at 64 UTF-8 bytes.
Formatting must return borrowed Text without errors. There is no fallback to a
different locale or dynamic resource loading. The fixed English provider lazily
initializes once and retains initialization failures without retrying.

The English resource contains 86 **candidate** classic VBA diagnostic titles
adapted from Microsoft's reference documentation. Its numeric inventory is the
intersection of the VBA trappable-error table and .NET's numbered runtime keys;
.NET message text is deliberately not used because its wording differs. Other
valid codes use the explicit unknown-message fallback. The source selection is
reference-based engineering evidence, not observed native Access mapping.
[Source hashes and attribution](../crates/query/tablecraft-expressions/data/error_messages/NOTICE)
accompany the catalog. The adapted data is CC-BY-4.0; original Rust and generator
code remain MIT. Retain the data's attribution/license in distributions including
compiled consumers. See [dependency records](DEPENDENCIES.md#localized-error-messages).

## References and remaining compatibility work

Microsoft's [Error function](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/error-function)
describes numbered lookup, undefined codes, zero and omitted-number state. The
[VBA error table](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/trappable-errors)
provides linked classic diagnostic titles; the
[Fluent bundle API](https://docs.rs/fluent-bundle/0.16.0/fluent_bundle/) supplies
catalog parsing and checked formatting.

The [Access query sandbox list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
includes Error/Error$, whereas Microsoft's
[Access function page](https://support.microsoft.com/en-gb/access/functions/error-function)
says sandbox disables the function. Native Access execution, sandbox behavior,
exact code coverage, host-defined/HRESULT mappings, localized wording and further
coercion rules remain unqualified. No native-completeness claim follows from the
reference English catalog or a Partial tracker entry.

## Verification

The expression/query batch passes 752 tests including doctests; a separate
parser-free build passes 285 tests and its dependency tree excludes Fluent.
All 34 SQLite tests plus its doctest pass, including request-local message
contexts. Sixteen selected CLI tests cover message lookup, closed Eval, CurrentUser,
Variant Error results and native queries. Clippy, warnings-denied API docs,
formatting, dependency boundaries and 32 Python guard tests pass.

The original `testing/compare_error_messages.py` passes 560 controls: all 86
reference messages through scalar/bound/closed Eval, zero/unknown/refusal cases,
and native-field/grouped integration on the already registered original
ScalarSamples V2000/V2007/V2016/V2019 files. Independent Jackcess getters supply
row IDs without evaluating SQL or VBA; all original file hashes remain unchanged.
This verifies catalog wiring and the declared profile, not native message results.

Reproduce with the current CLI and the reviewed Jackcess JAR, adding one or more
original database paths for native-field integration:

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-cli --locked
python3 docs/testing/compare_error_messages.py --cli target/debug/tablecraft \
  --jar /path/to/jackcess-5.0.4.jar --database /path/to/Scalar-V2016.accdb
```

The catalog generator accepts reviewed local VBA table/topic files and the .NET
numeric resource inventory. It performs no downloads. The generated provenance
hashes allow exact source verification; source extraction strips documentation
Error-number and duplicate-topic suffixes, not diagnostic wording.


## Signed numbers and typed Error$ identity (23 October 2026)

The dated [MS-VBAL specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
recorded in [declaration references](DECLARED_ARITHMETIC.md) explicitly permits
negative Error numbers, requires Long conversion and specifies Overflow above
65535. Its Error/Error$ declarations distinguish Variant from String returns.
The shared evaluator now follows those input rules. A separate checked message
code keeps CVErr construction and standard Error data unchanged.

The existing Error$ spelling now retains a distinct built-in/bound-plan identity
through traversal, grouping identity, dependencies and execution. Under the
explicit declaration profile it has Scalar String provenance; Error has Variant
provenance. Both use one lookup kernel and retain once-only callbacks, eager
failures and ordinary Text publication. Omitted-number runtime state stays
deferred; no new VBA-only function or scripting execution is added.

Flat Fluent resources may supply exact negative-code messages; otherwise the
selected catalog's explicit fallback applies. The bundled 86-title English data,
its bytes, provenance and license remain unchanged. Accepting a signed number
does not establish its native Access wording, HRESULT mapping or sandbox behavior.
Standalone numeric/NULL/code/output/cancellation limits still apply. Async SQLite
providers remain explicit and request-local.


## Deliberate expression-state admission (23 October 2026)

Error/Error$ now retain explicit expression states through both SQL evaluators
and admit their argument after the existing entry work/cancellation/depth check.
Error data fails TypeMismatch before catalog lookup, including a zero Error code;
it is not implicitly a numbered Long. Host Missing remains NonScalarExpression.
Host Empty follows Long Let-coercion to zero and therefore returns empty Text
without requiring or consulting a catalog. This does not add a SQL Empty
constructor or change the empty Text identity of query Nz(NULL).

The Value-only and expression-state APIs share the existing checked Long,
message-range, provider and output kernel. Explicit `Error(CLng(CVErr(11)))`
continues to look up code 11. NULL, signed limits, numeric Text policies, provider
failures and output limits retain their existing behavior. Error remains Variant
and Error$ remains Scalar String in the explicit declaration profile. No returned
message is interpreted as an operational error or as authority to run code.

The reference is [MS-VBAL revision 3.0](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74),
Error/Error$ and the Error/Empty Let-coercion rules (5.5.1.2.9/11). Operational
argument failures still propagate before admission, including eager unselected
IIf branches; volatile operands run once. Derived, subquery, encounter/domain
aggregate paths retain this boundary. Source-free and optional registered native
index-fixture controls in `compare_error_messages.py --native-directory PATH`
verify state refusals, explicit conversions, candidate catalog messages and
unchanged originals. This is not native Access SQL/message qualification.
