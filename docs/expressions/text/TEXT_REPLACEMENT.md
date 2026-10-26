# Bounded Replace expressions

`Replace(expression, find, replacement[, start[, count[, compare]]])` now resolves
in scalar SELECT evaluation and bound retrieval. It returns a String, replacing
non-overlapping source matches without searching the inserted replacement again.
The result begins at the requested start position: the preceding source prefix
is not retained. Grouping, nested calls, correlated fields and reusable parameters
use the existing text-call plan rather than a separate query runtime.

## Retained String operands and results

Replace now retains the source, find and replacement String operands through
both evaluators. Complete raw UTF-16 ranges, including isolated surrogates and
unaligned byte ranges, can participate without premature Unicode publication.
Binary matching uses unit spans; inserted units are never searched again.
General database modes use the separate explicit raw substring provider and
consume its actual expansion spans. Equality/LIKE support does not imply search
support, and provider failure never triggers a different matcher.

The result stays lossless through raw-aware consumers, derived cells, groups,
correlations, aggregates and domain retrieval. For example,
`AscW(Replace('😀',ChrW(55357),''))` returns -8704, while
`LenB(Replace('😀','z','',2,0))` returns 2. The suffix starts at the requested
unit even inside a pair. `Replace(ChrW(55296)&'a','a',ChrW(56320))` repairs a
surrogate pair and publishes U+10000 as valid final Text. Directly publishing
an unpaired result remains UnrepresentableText and buffered queries publish no
partial table.

The public `apply_retained_replacement` API returns a ByteOperand. Its borrowed
Text path retains existing scalar formatting and UTF-8 output accounting when
the inputs and suffix are representable. Other inputs use bounded unit buffers
and a metered raw-byte sink under the existing signed-Long String ceiling.
Final Text publication independently enforces UTF-8 limits. Materialized APIs
retain strict publication; no stored/source/parameter carrier changes.

Control conversion still precedes source/find/replacement admission. Every written
argument remains eager, including count-zero/empty-find cases. NULL remains an
error, Empty retains its explicit String/numeric targets and Error/Missing keep
their existing failures. All required operands must be admitted and raw byte
counts must be even before shortcuts. A shortcut requires no unused comparer.
A real nonbinary match requires an explicit raw provider or strict Unicode
publication through the existing provider path; system Text/LCID support is not
inferred from General database support. A raw provider range must be nonempty
and within the remaining suffix; success checks cancellation before use.

The [independent runner](../../testing/compare_raw_replacement.py) passes 9,068 controls:
Python nonoverlapping unit models, both CLI evaluators, independently keyed General
spans, raw output observations and sequential/selected-index queries over four
unchanged native inputs. The Java helper uses the reviewed external encoder and
existing original span model; TableCraft alone evaluates SQL. Default resource
limits remain unchanged; smaller projection batches keep the recipe within them.
The existing count-conversion, scalar-source, decoded General and raw General
search recipes also pass. These are engineering model checks; native Access
coercion, defaults, locale/code-page behavior and raw edge rules remain unqualified.

## Arguments and current policies

Explicit General CLI policies now supply database-mode match spans through the
[General search provider](../../access/indexes/NATIVE_TEXT.md#general-substring-spans). For example,
`Replace('SS ß','ß','x',1,-1,2)` returns `x x` with `--collation general-2010`:
the two-character and one-character source matches consume their actual spans.
The shortest nonempty equivalent span wins; ignored prefixes/trailing-space
equivalence follow that explicit model. Inserted text is never searched again.
The omitted binary mode stays unchanged, and text-mode/LCID policies remain
unsupported. Independent short-key comparisons are separate from native Access
substring semantics and tie-breaking.

| Argument | Implemented behavior |
| --- | --- |
| expression, find, replacement | Required string operands; SQL evaluation formats ordinary scalars through explicit CStr policies. NULL remains InvalidNullConversion; Binary/GUID are TypeMismatch. The pure kernel retains Text/NULL carrier validation. |
| start | One-based UTF-16 position; omitted means 1. Checked numeric/legacy DATE-to-Int32 half-even conversion; SQL numeric Text uses the explicit number policy. Zero/negative starts fail. |
| count | Checked numeric/legacy DATE-to-Int32 conversion, plus explicit-policy SQL Text; omitted/-1 means unlimited, zero means no replacements, positive values limit matches. Values below -1 fail. |
| compare | Shared -1/0/1/2 mode conversion; omitted selects the explicit current binary profile. -1 requires a declared host Option Compare policy. |

SQL missing optional slots retain these defaults; they never replace explicit
SQL NULL. Required slots, unsupported arities, wildcards and qualifiers fail before
operand evaluation. Every written operand evaluates eagerly, including unused
count-zero/empty-source arguments. An error in a later argument remains visible
when an earlier operand is NULL. After evaluation, optional argument validation
precedes text-carrier validation under this explicit current error-order policy.

Empty source or start beyond its length returns empty Text. Empty find or count
zero returns the unchanged suffix beginning at start. Empty replacement deletes
matched source text. UTF-16 offsets preserve supplementary-character widths;
a start inside a surrogate pair can now retain the exact suffix through the
lossless evaluator path described below. Strict Text publication still rejects
an unpaired final result instead of changing boundaries or emitting replacements.

```sh
tablecraft query eval "SELECT Replace('banana','an','X') AS Replaced"
tablecraft query run "SELECT Replace('banana','an','X',3,1) AS Suffix"
tablecraft query run "SELECT Replace('banana','an','X',,,2) AS Replaced" --collation ordinal
```

The respective Text values are `bXXa`, `nXa` and `bXXa`. CLI TSV retains its existing
quoted display cells and CSV transport escaping.

## Search, construction and limits

Mode 0 scans exact UTF-16 matches at valid Unicode boundaries without normalization.
Text/database modes use [the separate search provider](STRING_SEARCH.md), including
unequal source/needle span lengths. The provider receives the remaining source
suffix and must select its earliest match. Returned ranges must consume a nonempty
span, stay inside the source and map to representable Unicode boundaries. Invalid
ranges fail; a zero-width callback result cannot cause an unlimited loop.
CLI ordinal supplies database/default exact matching, not a locale-aware text mode.

The parser-free kernel returns a borrowed `Replacement` recipe. Its `write` method
streams borrowed gaps/replacements to a host `TextOutput`; it allocates neither a
match list nor an intermediate result. Inputs, candidates and comparisons consume
work through the sink; callbacks retain their host-owned work/cancellation contract.
The binary candidate scan is bounded by work checks, not claimed to be linear-time.

The evaluator sink checks cancellation and the per-value UTF-8 length ceiling,
charges cumulative payload bytes, then reserves output storage fallibly before
appending. Capacity may grow geometrically; the byte budget measures payload work,
not exact allocator capacity/resident memory. Failed work/byte charges remain
consumed. On any error the evaluator discards its private partial String; CLI output
is published only after complete query success. Direct library sinks must likewise
discard partial output on failure. Existing input-size/depth limits remain in force.

## Evidence and remaining work

The implementation follows Microsoft's [Access Replace reference](https://support.microsoft.com/en-us/access/functions/replace-function)
and [VBA Replace reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/replace-function)
for suffix construction, counts, empty strings and NULL-source rejection. Defaults,
conversion/error precedence and NULL find/replacement handling above are explicit
current policies, not native Access observations.

Five kernel tests include generated Unicode `replacen` comparisons, suffix/count/
deletion/overlap controls, a provider with unequal-length matches, invalid/nonadvancing
provider ranges and bounded expansion/cancellation. An evaluator test checks work,
per-value and cumulative byte failures during construction. Two query tests cover
binding, grouping, correlations, parameter reuse, defaults and eager failures. Two
CLI tests decode TSV with the csv crate and check both results and buffered errors.
The [independent recipe](../../testing/compare_replacement.py) uses a UTF-16 list model,
explicit binary/ordinal modes, missing defaults, surrogate refusals, resource-limit
refusals and independently decoded sequential/indexed native rows. Source bytes
remain unchanged; see [implementation status](../../IMPLEMENTATION.md) for check totals.

Complete Variant coercion, native code-page/locale matching, native default/error precedence and actual Windows Access
qualification remain open. `Replace` is Partial in the
[built-in tracker](../ACCESS_BUILTINS.md). The API does not execute VBA or stored code.

The shared `TextOutput` contract now lives in `functions::text::output` and also
serves [repeat/reverse construction](TEXT_CONSTRUCTION.md). The existing
`replacement::TextOutput` import remains supported by a re-export.

## Contextual start and count conversion

Replace now accepts legacy DATE start/count values using their raw serials and
the existing checked Long conversion. Numeric Text positions use the selected
NumberFormat in SQL evaluation. Half-even rounding precedes domain checks:
`Replace('banana','an','X','2,5','2,5')` under de-de returns `XXa`. A DATE count of
-1.25 rounds to unlimited -1, while -0.5 rounds to zero replacements. These
numeric policies reuse [the destination conversion kernels](../dates/DATE_NUMERIC.md),
without calendar interpretation, machine locale or a new decimal implementation.

The evaluator converts only the optional start/count slots into stack numeric
values. The recipe still borrows the original source, find and replacement
strings. Pure text kernels, including apply_with_context, still reject numeric
Text positions rather than allocate unmetered normalization buffers. SQL missing
slots retain the existing defaults; an explicit NULL remains an error.

The current validation order is start conversion, count conversion, comparison
mode, numeric domains and then string carriers. A NULL source never skips the
earlier validations. All written expressions evaluate eagerly before that order
is applied, so a late division error remains visible even with malformed Text
positions. Source coercion remains unchanged. [Comparison modes](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
now also accept raw DATE and explicit-policy SQL numeric Text, retaining their
closed codes and original validation order. Empty find/count-zero
continues to return the suffix; inserted replacement text is never searched again.
Materialized Text APIs still refuse an unpaired result. The retained evaluator
path below can observe exact suffix units, including with zero replacements.
Native NULL/error/subtype precedence remains provisional pending Access comparison.

Normalization buffers, match scans and output share cumulative limits. A nine-byte
control permits two three-byte count normalizations plus the three-byte result
while borrowing all three original Text inputs. Every observed conversion,
match-scan and output checkpoint is cancelled independently. No source clone,
match list, runtime or dependency is introduced.

The batch passes 819 expression/query tests including doctests, 307 separately
run parser-free expression tests and three selected CLI tests. All-feature/minimal
Clippy, warnings-denied rustdoc and format/diff checks pass. The original
[comparison recipe](../../testing/compare_replace_counts.py) passes 798 independent
Decimal/strict UTF-16/Python replacement controls, including both CLI commands,
en-us/de-de policies, rounded sentinels and unchanged Jet 4/ACE 2007/ACE 16-family
ID getter integration. Only TableCraft executes SQL. Original/JAR hashes are
checked; no third-party data/code is added and no native Access SQL comparison
was run. Replace remains Partial.


## Scalar string operands

SQL evaluation now converts ordinary non-Text expression/find/replacement values
through the shared [CStr service](CSTR.md). Numeric values require NumberFormat,
date values require DateTextFormat and Boolean spellings use the existing explicit
profile. Text remains borrowed. Error/Empty/Missing states are not admitted by
this ordinary scalar path; no formatter/provider is activated by inspecting files.

Optional start/count conversion and mode/domain validation finish before string
conversion. Required operands then convert in source/find/replacement order,
including for empty source or count zero. NULL is an error rather than a default
or empty string. Every written expression has already evaluated eagerly, so a
later division failure is not hidden by an earlier NULL or unused argument.

Validated owned options are separate from borrowed string operands. Converted
scalar strings live only during recipe execution; the recipe streams into the
existing metered sink and publishes only a complete owned Value. This adds no
cloned input array, match list, second formatter or cached prepared spelling.
CStr conversions and replacement output share allocation/work/Text limits and
cancellation checks. Pure Text inputs retain their existing borrowing behavior.

The [Access Replace reference](https://support.microsoft.com/en-au/access/functions/replace-function)
and [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
provide the string-operand and regional conversion basis. This implementation
reuses TableCraft's materialized CStr profile; complete native Variant semantics,
regional formatting and exact Access error precedence remain unqualified.

Three expression and three query tests cover numeric/fixed/date/Boolean operands,
policy rebinding, grouping/correlation, eager/NULL/option failures, Text/byte limits
and every observed cancellation checkpoint. The original
[scalar runner](../../testing/compare_replacement_scalars.py) passes 336 authored numeric
spelling/UTF-16 controls against an independent replacement model and raw IDs read
by Jackcess from the unchanged four-family 20,000-row inputs. Only TableCraft
evaluates SQL.


## Ordered expression-state admission (23 October 2026)

Replace now captures expression states eagerly and admits start, count, comparison
mode, numeric domains, source, find and replacement in its existing validation
order. Invalid controls precede source-state admission; source conversion precedes
find/replacement conversion. Error data fails TypeMismatch, and host Missing stays
NonScalarExpression. A NULL source remains InvalidNullConversion after valid
controls. Empty-source, empty-find and count-zero output shortcuts still require
all string operands to pass their earlier conversion/admission steps. Later
operational argument failures remain eager; a volatile argument runs once.

Host Empty uses empty Text in the first three positions and Long zero in numeric
options. Empty replacement deletes matching text. Empty start remains Domain;
Empty count means zero substitutions, distinct from omitted unlimited count;
Empty mode selects binary comparison. No SQL Empty constructor, stored state or
ambient locale is added. Explicit CStr(CVErr(...)) remains separate from implicit
string admission, and query Nz(NULL) retains its Text identity.

Value-only and expression-state APIs share option validation and the original
borrowed replacement recipe/output sink. Raw input/output limits, UTF-16 suffix
and surrogate boundaries, host consumed spans and resource/cancellation checks
are unchanged. No match-list or normalized argument-list allocation is added.
Native NULL/error precedence, full Variant coercion and locale behavior remain
unqualified.

Microsoft's [Access Replace query example](https://support.microsoft.com/en-us/access/functions/replace-function)
uses SELECT over ProductSales, establishing direct query availability separately
from the VBA argument/coercion reference. The independent state runner and scalar
replacement runner retain registered native input hashes before/after reads;
neither executes Microsoft Access SQL or qualifies native saves.
