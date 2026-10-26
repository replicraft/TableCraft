# Explicit Unicode text casing

`LCase(Text)` and `UCase(Text)` now execute through the parser-independent text
slice, scalar evaluator and read-only query engine. Both remain **Partial** in
[the built-in tracker](../ACCESS_BUILTINS.md). Their current policy uses ICU4X Unicode
casing, independently of database text ordering. Windows Access/NLS qualification
is still pending; ICU output is not evidence of identical ACE behavior.

## Language and input contracts

Hosts parse a `CaseLocale` and supply it through `Evaluator::with_case_locale` or
`ExecutionOptions::case_locale`. The optional SQLite worker owns a request-local
copy in `AsyncOptions`; subsequent requests never inherit a previous language.
The wrapper keeps upstream ICU types private. No system locale is discovered.

The CLI accepts `--case-locale LANGUAGE` on `query eval` and `query run`:

```sh
cargo run -p tablecraft-cli -- query eval \
  "SELECT LCase('Hello World 1234'), UCase('straße')" --case-locale en-US
cargo run -p tablecraft-cli -- query run \
  "SELECT LCase('Iİiı'), UCase('Iİiı')" --case-locale tr
```

Language identifiers accept language/script/region/variants, case-insensitively,
with a 128-byte ASCII policy-input ceiling. Underscores, Unicode extensions and
private-use extensions are rejected. `und` explicitly selects ICU root behavior.
ICU's selected-language rules apply where available; otherwise ICU root rules
apply. A syntactically valid identifier does not certify a Windows locale or
translation availability. Formatting and comparison settings remain separate.

The pure kernels accept Text and NULL. SQL evaluation also converts ordinary
scalar operands through explicit CStr policies; see [scalar sources](#scalar-sources).
NULL returns NULL without a language. Non-NULL Text, including converted numeric
or Boolean text and empty text, requires a supplied language. Exactly one argument
is accepted; `$` aliases and qualified VBA calls are not implemented.
Ordinary eager argument/error evaluation remains unchanged.

## Mapping and limits

ICU handles contextual whole-string mapping, including final Greek sigma,
Turkish/Azerbaijani dotted I and Lithuanian combining accents. Mappings can change
length: root uppercasing `straße` returns `STRASSE`; root lowercasing U+0130 emits
`i` followed by U+0307. Supplementary characters, embedded NUL, nonletters and
combining sequences remain valid Unicode; no normalization is applied. Casing
neither supplies SQL equality nor grants permission to remove the original filter.

`Casing` is a borrowed recipe. It uses `CaseMapper::new()` with compiled data and
`Writeable::write_to`, never an unmetered intermediate output String. A private
fmt::Write bridge retains the original cancellation/allocation/byte-budget error.
Evaluator output is fallibly allocated and metered before each fragment append.
Source input has the existing per-value byte ceiling; output expansion can fail
that ceiling even when the input fits. No partial result reaches CLI stdout.

ICU context iterators cannot accept our cancellation callback directly. Before
mapping, the recipe reserves four complete byte-length scans through the host's
step callback. Reviewed non-Greek rules stop context scans at cased/base/above
characters, limiting their aggregate traversals. Greek uppercase can repeatedly
scan diacritic runs; it additionally reserves four full input scans for every
non-ASCII scalar. This deliberately conservative charge can reject long Greek
uppercase inputs before their byte ceiling. Source/context preflight and output
fragments check cancellation; callbacks do not interrupt an individual ICU lookup.
Changes to the upstream algorithm require reviewing this accounting again.

The generic TextCall keeps language selection at execution time, including
parameters, grouped expressions, aggregates, correlations and retained unused
computed expressions. No native source pushdown, UPDATE support, case-folding
collation, database locale inference or runtime dependency is added.

[StrConv](STRCONV.md) now shares this output bridge and language policy for
upper/lower casing. Its proper-case recipe uses documented separator-delimited
segments and ICU first-cased-character titlecasing with a lowercased suffix.
An explicit Japanese language also recognizes U+3000 IDEOGRAPHIC SPACE, preserving
it as a boundary; other languages retain the ASCII separator profile. NBSP and
EM SPACE do not become separators. See [Japanese proper case](STRCONV.md#japanese-proper-case-word-boundaries).
It retains the same conservative context preflight and output accounting; this
addition does not qualify native Windows word-start or titlecase behavior.

## Evidence and remaining scope

Focused kernel tests cover documented ASCII examples, language/context differences,
expansion, nonletters, NULL/type/arity/policy validation and injected sink failures.
Generated ASCII tests compare integer-offset mappings. Evaluator tests exercise
expansion, cumulative bytes, work and cancellation; query/CLI/worker tests exercise
request isolation, grouping, correlation and buffered errors.

The [independent recipe](../../testing/compare_casing.py) compares established Unicode
characters against Python casing plus explicit language goldens. Its optional
native-row controls compare read order/row membership with unchanged Jackcess
inputs while executing casing in TableCraft. These are independent model/reader
checks, not native expression comparisons. Native Access tests must still establish
Windows locale/version mappings, length changes, Variant coercions, aliases and
error order before broad compatibility claims. No performance gain is claimed.

References:

- [Microsoft LCase](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/lcase-function)
- [Microsoft UCase](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ucase-function)
- [ICU case mapper](https://docs.rs/icu_casemap/2.3.0/icu_casemap/struct.CaseMapperBorrowed.html)
- [ICU4X source](https://docs.rs/crate/icu_casemap/2.3.0/source/src/internals.rs)


## Scalar sources

SQL LCase/UCase now reuse the shared CStr service for ordinary non-Text scalar
operands before invoking the existing casing path. Numeric values require an
explicit NumberFormat, dates require DateTextFormat and Boolean values retain
the existing English spelling. Converted non-NULL values still require CaseLocale.
No system locale, database sort order or previous query policy supplies it.
Binary/GUID remain refused. [Expression states](CSTR.md#unary-string-expression-states)
now admit host Empty as empty Text while keeping Error/Missing refusal explicit.
NULL still returns NULL after eager argument evaluation.

Dispatch converts once and re-enters with proven Text. Text/NULL inputs keep the
existing path, with no extra source copy. Source formatting and final ICU output
share cumulative allocation/work, per-value Text and cancellation limits. Converted
spellings are operation-local, so prepared queries use each execution's actual
formatting/language settings. The existing ICU mappings and explicit native NLS
qualification gaps remain unchanged.

The same scalar conversion now applies to LTrim/RTrim/Trim. Trimming removes only
U+0020 spaces; tabs, nonbreaking spaces and other controls remain. The pure kernels
stay Text/NULL and introduce no formatting dependency.

Microsoft's Access references for
[LCase](https://support.microsoft.com/en-us/access/functions/lcase-function),
[UCase](https://support.microsoft.com/en-us/access/functions/ucase-function) and
[trimming](https://support.microsoft.com/en-us/access/functions/ltrim-rtrim-and-trim-functions),
together with [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554),
provide the string-operand/conversion basis. This materialized profile does not
establish complete Variant coercion, native regional spelling or error precedence.

Three expression and two query tests cover scalar source kinds, nested/reused
execution settings, NULL/provider failures, preserved whitespace, byte quotas and
every observed conversion/output cancellation checkpoint. The original
[scalar runner](../../testing/compare_trim_case_scalars.py) passes 336 authored spelling
and ASCII-only casing controls, with raw IDs read independently from four unchanged 20,000-row
Jet 4/ACE inputs through sequential and both index directions. Its ASCII oracle
is separate from broader ICU/Windows mapping qualification.

The existing independent casing runner also passes 562 stable-Unicode, language
and refusal controls in scalar CLI mode. Its optional native-row branch was not
rerun; native coverage for this slice is the four-family scalar runner above.

## Retained String trimming

LTrim/RTrim/Trim now retain complete raw UTF-16 String units through both
evaluators. They remove U+0020 at the selected edges while preserving isolated
surrogates, NUL, tabs, nonbreaking spaces and other units. Active ranges can begin
at an unaligned byte offset; units are read as little-endian from that range.
Odd byte counts refuse trimming instead of dropping or padding a byte.

The pure `byte_string::trim_spaces` kernel selects a range in the owned ByteString
without copying its payload or decoding Unicode. It checks work/cancellation
before each examined edge unit. Already adopted inputs need no payload allowance;
constructors and query-cell copies retain their own existing charges. Ordinary
Text/NULL and scalar sources keep their borrowed Text path, explicit formatting
policies and output limits. Error/Missing and unsupported carriers remain refused;
Empty targets empty Text. No locale or comparer is needed merely to trim.

For example, `LenB(Trim(' '&ChrW(55296)&' '))` returns 2.
`Trim(' '&ChrW(55357)&' ')&ChrW(56832)` repairs a pair and returns valid final Text.
A directly selected unpaired result still fails before buffered output reaches
the user. Derived/grouped/correlated cells, prepared values, domain retrieval
and nested Replace can retain those intermediates. Trimming keeps its existing
fresh result text policy; it does not activate an unresolved input comparer.
Stored values, source/parameter carriers and write gates are unchanged.

Microsoft's specific [trimming query examples](https://support.microsoft.com/en-us/access/functions/ltrim-rtrim-and-trim-functions)
establish direct query availability. The complete-unit/range rules here are the
explicit engineering profile; native Access edge/error and coercion qualification
remain separate.

The original [runner](../../testing/compare_raw_trimming.py) passes 3,786 independent
controls. Python list prefix/suffix removal checks lengths and exact binary
result units across both CLI evaluators. Native getters supply four unchanged
inputs for sequential/selected-index length controls under stored and explicit
General choices. TableCraft alone evaluates SQL. Existing scalar trimming/casing
and retained-cell recipes pass 336 and 1,053 controls. No new dependency,
benchmark result or Windows Access comparison is supplied by this batch.
