# Numbered error-message expressions

`Error(number)` and `Error$(number)` return Text describing an explicit standard
error code. They neither raise an error nor construct Variant Error data. For
example, the CLI's selected English catalog returns `Division by zero` for
`SELECT Error(11)`. `Error(0)` returns empty Text without consulting a catalog.
Omitted-number `Error()` depends on the latest VBA runtime error and remains
unimplemented with the deferred `vba-rs` runtime. TableCraft's typed operational
failures never become that state. See [Variant Error values](VARIANT_ERRORS.md)
for the separate CVErr constructor and explicit numeric conversion.

## Execution contract

The explicit argument uses checked Long conversion, including half-even numeric
rounding, followed by the standard 0..65535 range check shared with CVErr. NULL,
invalid types, overflow and out-of-range codes fail. Numeric Text needs the
existing explicit number-format policy. Error data needs an explicit numeric
conversion; it is not silently interpreted as a code. Argument evaluation remains
eager, and unsupported arity is rejected before operands are evaluated.

Nonzero lookup requires caller-issued `ErrorMessages` in scalar Evaluator or
query ExecutionOptions. The provider must be immutable, deterministic and bounded
for one execution, with no external effects. It returns a borrowed message and
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

## Optional Fluent catalog

The `tablecraft-expressions/fluent-catalog` feature implements ErrorMessages using
fluent-rs (`fluent-bundle`, `fluent-syntax`) and explicit `unic-langid` identifiers.
The feature is enabled by the CLI; minimal expression consumers need not include
it. Only flat Text messages named `error-N` for canonical nonzero u16 codes and a
required `error-unknown` fallback are accepted. Duplicate IDs, attributes, terms,
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
