# Explicit Unicode text casing

`LCase(Text)` and `UCase(Text)` now execute through the parser-independent text
slice, scalar evaluator and read-only query engine. Both remain **Partial** in
[the built-in tracker](ACCESS_BUILTINS.md). Their current policy uses ICU4X Unicode
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

Only Text and NULL carriers are accepted. NULL returns NULL without a language;
non-NULL Text, including empty text, requires a supplied language. Other carriers
return TypeMismatch instead of guessing Variant string conversion. Exactly one
argument is accepted; `$` aliases and qualified VBA calls are not implemented.
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

## Evidence and remaining scope

Focused kernel tests cover documented ASCII examples, language/context differences,
expansion, nonletters, NULL/type/arity/policy validation and injected sink failures.
Generated ASCII tests compare integer-offset mappings. Evaluator tests exercise
expansion, cumulative bytes, work and cancellation; query/CLI/worker tests exercise
request isolation, grouping, correlation and buffered errors.

The [independent recipe](testing/compare_casing.py) compares established Unicode
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
