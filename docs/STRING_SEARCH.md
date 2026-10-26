# InStr and InStrRev substring search

The shared text kernel implements bounded substring search in scalar evaluation
and retrieval. Positions are one-based Int32 UTF-16 code-unit offsets; no match
returns zero and a NULL string returns NULL. This counts supplementary characters
as two units, preserves embedded NUL/spaces and does not normalize Unicode.

## Retained binary String operands

InStr and InStrRev now retain their two String operands in both evaluators.
Binary mode searches complete UTF-16 units directly, including isolated surrogate
units, mixed decoded/raw strings and unaligned active byte ranges. For example,
`InStr(1,'😀',ChrW(56832),0)` returns 2, and
`InStrRev(ChrW(55296)&'a'&ChrW(55296),ChrW(55296))` returns 3.
No payload allocation, Unicode replacement or byte realignment is needed for
already supplied binary String inputs. Scalar formatting retains its own budgets.

Starts/results remain unit positions; InStrB remains a separate byte search.
Reverse bounds still require the complete match to end within the requested
prefix. Odd byte counts refuse non-NULL searches, including empty-needle or
out-of-range cases. Valid controls followed by a NULL String return NULL without
decoding its partner. Later written evaluation failures remain eager.

Explicit binary mode and a host-declared binary default select this path.
InStrRev's omitted comparison remains binary. Database providers can explicitly
accept raw units as described below. Other provider searches still require
strict Unicode publication and their existing substring providers;
raw binary support does not make an equality comparer into a search provider.
Provider failures are propagated without a binary retry. Final String output
and stored/source/parameter contracts are unchanged.

Microsoft's specific [InStr query examples](https://support.microsoft.com/en-gb/access/functions/instr-function)
and [InStrRev query examples](https://support.microsoft.com/en-gb/access/functions/instrrev-function)
establish direct query availability. The unit model and current edge/error rules
remain independent engineering behavior pending native Access qualification.
The [independent runner](testing/compare_raw_string_search.py) passes 3,634
controls using Python integer-list windows, both CLI evaluators, forward/reverse
bounds, failures and four unchanged native inputs through sequential and selected
primary-index reads. Inert native getters supply rows only; TableCraft evaluates
SQL. General search/replacement and database StrComp regressions also pass.
No benchmarks or Windows Access comparisons were run.

## Retained database String operands

Providers can now opt into `StringSearchProfile::Utf16` for a specific
`StringCompareMode` through `Collation::string_search_profile`, then implement
`find_utf16`. Equality and LIKE profiles do not activate substring search.
The callback returns a consumed unit span under the same earliest/latest,
forward/reverse bounds and cumulative work/cancellation contracts as `find`.
Invalid ranges and callback failures refuse the result; neither triggers a
Unicode, binary or alternate-provider retry.

For retained raw inputs, the evaluator validates controls and NULL before
converting both String sources into budgeted unit buffers. Complete units can
include isolated surrogates or begin at an unaligned byte range. Odd byte counts
still fail when search is required. Source formatting, eager errors and strict
final String publication retain their separate contracts. Ordinary decoded inputs
retain the existing decoded provider path.

The General adapter and explicit CLI General policies now opt in for Database
mode only. Mode 2 and host-declared Database defaults share the existing
full-weight span model. The standalone engine exposes `find_general_utf16`;
its decoded/raw paths share pruning, shortest-span selection and SQL trailing-
space rules. Valid surrogate pairs remain single boundary elements; isolated
units remain individual elements with their family-specific weights. This is
model consistency, not native Access locale/search qualification.

`query run --collation general-2010` can now evaluate
`SELECT InStr(1,ChrW(55296)&'Straße','ss',2)` and returns 6.
The source-free library evaluator accepts the same explicit host contract;
CLI `query eval` retains its existing no-collation interface. Text mode and
explicit LCID raw search remain separate; the General provider does not supply
a system-locale implementation. Built-in calls use the supplied default provider,
while SQL predicates continue to route stored field policies separately.

The original [raw General runner](testing/compare_raw_general_search.py) passes
4,544 controls. Its Java helper uses the reviewed independent encoder with the
existing exhaustive short-span model, including unpaired units, valid pairs,
ignored keys, accents, expansions, spaces and both bound directions. Native-source
controls read four unchanged inputs sequentially and through selected primary
indexes. Original binary and decoded General regressions also pass. No native
Access comparisons or new performance claims follow from these checks.

## Arguments and bounds

| Call | Current behavior |
| --- | --- |
| InStr(haystack, needle) | Start at position 1; use an explicit host Option Compare policy. |
| InStr(start, haystack, needle[, compare]) | Start must round to a positive Int32. An explicit compare argument requires a written start. |
| InStrRev(haystack, needle[, start[, compare]]) | Omitted/-1 start uses the final UTF-16 position; other starts must round to a positive Int32. Omitted compare defaults to binary. |

Supplied starts use checked half-even numeric/legacy DATE-to-Int32 conversion;
SQL numeric Text starts additionally use the explicit NumberFormat. Mode operands
retain their separate checked numeric conversion and closed/LCID dispatch rules.
Explicit NULL in either optional numeric argument is InvalidNullConversion, never
an omitted default. Invalid start/mode values remain errors even for NULL strings.
All written operands evaluate eagerly; a division error in a later argument is
not hidden by a NULL earlier operand. SQL evaluation formats ordinary non-Text
scalar string operands through its explicit CStr policies; the pure Text/NULL
kernel retains its strict carrier contract. See [scalar operands](#scalar-search-operands).

An empty haystack returns zero. A positive start past its UTF-16 length returns
zero. Otherwise an empty needle returns the effective start position. Reverse
search includes only matches whose entire UTF-16 span ends at or before start;
it does not merely limit where a match begins. Overlapping matches are allowed.
Empty-needle/beyond-end precedence and native conversion/error ordering remain
explicit current policies requiring Access qualification.

SQL optional slots are lowered by one shared, stack-only normalizer, without
cloning expression trees. A missing trailing comparison argument means omission.
InStr's missing leading start is accepted when no explicit compare is supplied.
InStrRev accepts missing start with a supplied comparison mode and missing trailing
options. Required strings cannot be omitted. These defaults never turn a written
SQL NULL into a value. Unqualified names alone resolve; VBA/UDF activation is absent.

## Comparison and resource policies

Mode 0 uses an allocation-free UTF-16 scan with cancellation/work checks for input
measurement, candidate positions and comparisons. Adversarial repeated-prefix
searches can be quadratic, but consume the caller's step budget and terminate
when it is exhausted. This is a bounded correctness implementation; no throughput
or native code-page equivalence claim is made.

Mode 1 delegates to the explicit text provider; mode 2 delegates to the explicit
database provider. Mode -1 uses the declared host Option Compare policy.
InStr's omitted mode also uses that policy; InStrRev's omitted mode stays binary.
The [-1/0/1/2 mode conversion contract](STRING_COMPARISON.md) is shared.
`InStr` additionally accepts a structurally valid positive LCID in its comparison
argument and dispatches it to the separate `Collation::find_locale` callback.
Numeric conversion remains checked half-even Int32 conversion. `LocaleId` retains
the full language/alternate-sort identity and rejects zero/reserved high bits;
structural validity does not establish provider support. Unknown/custom/default
IDs must be explicitly resolved by the host, with no ambient locale discovery.
Standard numeric mode codes retain their existing meaning. `StrComp`, `InStrRev`
and `Replace` retain the documented closed -1/0/1/2 mode set.

Microsoft's [InStr reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instr-function)
explicitly permits LCIDs; the other functions' mode tables do not document that
extension. The bit structure comes from
[MS-LCID](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/63d3d639-7fd2-4afb-abbe-0d5b5551eef8).
This is a working explicit host dispatch contract, not a built-in implementation
of each Windows locale. Existing ordinal/General CLI providers intentionally do
not implement `find_locale`; nonempty comparisons that require it return
CollationRequired. NULL/empty/out-of-range cases retain their existing rules.

`Collation::find` is a separate search callback. Merely supplying equality/order
comparison does not implement locale substring matching. Its `StringSearch`
boundary is either a zero-based UTF-16 forward start or exclusive reverse end.
Providers return a UTF-16 match range, allowing expansions/contractions where
source and needle lengths differ. Returned bounds are validated before emitting
a position; invalid ranges return InvalidTextMatch. Providers own callback work,
cancellation, match correctness and earliest/latest selection. The same bounds
validation applies to `find_locale`. Input work is charged before dispatch and a
step/cancellation checkpoint follows either callback before accepting its range.
A reverse callback result must also start strictly before its exclusive bound:
a result beginning at that bound would publish a one-based position beyond the
caller's requested start. This applies to decoded and opted-in raw UTF-16
callbacks under Text, Database and host-default modes. Interior zero-length
provider spans remain admitted, as do the existing forward-span rules; this
check does not decide native matching of nonempty ignorable text. A genuinely
empty needle keeps its earlier shortcut and does not invoke the callback.
Callback errors and the post-callback cancellation/work checkpoint remain ahead
of result acceptance, without alternate-provider retries.

Microsoft's [InStrRev reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instrrev-function)
describes searches bounded by the supplied start, and its
[Access query examples](https://support.microsoft.com/en-gb/access/functions/instrrev-function)
establish direct query use. The boundary validation is an explicit provider
contract correction, not native locale/zero-weight equivalence evidence.
No automatic fixed-width equality loop stands in for locale search.

CLI `--collation ordinal` supplies exact, case-sensitive substring matching for
database/default modes, with UTF-16 input/output boundaries. It handles boundaries
inside surrogate pairs without slicing invalid UTF-8. It does not provide locale
mode 1. Existing text-size ceilings, shared budgets and buffered output remain.
Explicit General policies now supply a bounded database-mode span provider for
InStr and explicitly selected database InStrRev. The
[General model](NATIVE_TEXT.md#general-substring-spans) handles unequal-length
expansions/composed accents, selecting the earliest/latest valid start and its
shortest nonempty matching span under the complete weights. UTF-16 positions,
shared cumulative budgets and module/default contracts remain explicit. Concrete text-mode
and LCID providers remain unsupported by the CLI; the chosen model is not native SQL
qualification. The independently maintained key encoder agrees on 268 short
forward/reverse/replacement controls across both General families.

```sh
tablecraft query eval "SELECT InStr(1,'banana','an',0) AS FirstMatch, InStrRev('banana','an') AS LastMatch"
tablecraft query run "SELECT InStr('banana','an') AS FirstMatch" --collation ordinal
```

The first command returns 2 and 4; the second returns 2.

## Evidence and remaining compatibility work

Microsoft's [InStr reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instr-function)
and [InStrRev reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/instrrev-function)
guide argument order, defaults, results and reverse end-boundary handling.
Their rules are distinct; neither implies native equivalence of Rust string
search or an inferred system/database locale.

Five kernel tests, including generated independent UTF-16 window matching, cover
Unicode positions, overlap, bounds, NULL/error behavior, separate locale search,
invalid provider ranges and cancelled scans. Two query tests cover grouping,
correlation, reusable parameters, optional slots and eager failures. Two CLI tests
cover output carriers, ordinal policy, explicit defaults and buffered refusals.
The [original reference recipe](testing/compare_string_search.py) passes 284 controls
against Python UTF-16 window models and independently decoded native table rows,
including sequential/indexed queries. Original database bytes remain unchanged.

Native Access execution, code-page/locale search, full Variant/string coercion,
concrete LCID providers and lossless unpaired-surrogate inputs remain open. This is `Partial`
in the [built-in tracker](ACCESS_BUILTINS.md), not full native Access qualification.

[Replace](TEXT_REPLACEMENT.md) also consumes the search provider, advancing through
non-overlapping suffix spans and requiring nonempty consumed matches for progress.
It charges construction separately through a bounded output sink.

## Contextual search starts

InStr/InStrRev now accept raw legacy DATE starts through the existing Long
conversion, and SQL numeric Text starts under the execution-local number format.
For example, `InStr('2,5','ababab','ab',0)` under de-de returns 3; the count rounds
to position 2 before the forward scan. Reverse `-1` remains the last-position
sentinel after conversion, including a DATE serial that rounds to -1. Date numeric
conversion uses the stored serial, without calendar conversion or zone lookup.

Preparation retains borrowed haystack/needle operands and validates the supplied
comparison mode before converting the start. Invalid/NULL starts remain errors
even for NULL string operands; this differs from repetition's existing NULL
shortcut. Missing Text number policy fails explicitly. No source strings are
cloned to replace a start argument. Count normalization and scans use cumulative
budgets and every observed contextual search checkpoint can cancel. A direct
control allows only three normalization bytes while searching a six-byte source.
Comparison modes, provider dispatch, UTF-16 spans, omitted defaults and raw-byte
InStrB retain their own profiles; Source coercion remains unchanged; [Text modes](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
now use the explicit number policy while retaining the same provider rules.

The [combined repetition/search evidence](TEXT_CONSTRUCTION.md#contextual-repetition-counts)
records 812 expression/query tests, 306 minimal-profile tests, four selected CLI
tests and 1,200 independent controls. The independent model enumerates UTF-16
windows and reverse end bounds, including empty needles and rounded sentinel/
invalid starts. Native Access comparisons remain unrun.


## Scalar search operands

The evaluator now uses the same bounded scalar string conversion as StrComp for
InStr/InStrRev haystack and needle operands. Text is borrowed without another
copy. Numeric values require NumberFormat, date values require DateTextFormat,
and Boolean values use the existing English True/False spelling. Binary/GUID
remain TypeMismatch. Expression states use the separate admission profile below;
no ambient regional setting is read.

Comparison mode and start conversion/domain validation precede operand formatting.
A NULL string then returns NULL without needing formatting for its partner.
All written argument expressions have already evaluated eagerly. Non-Text operands
materialize through the existing CStr service under shared bytes/work, selected
per-value Text limit and cancellation checks. Pure Text searches still borrow both
strings, and no formatted value is cached across prepared executions. Search units,
reverse bounds, empty cases and host/LCID provider dispatch remain unchanged.

Microsoft's [Access InStr reference](https://support.microsoft.com/en-us/access/functions/instr-function),
[MS-VBAL InStrRev declaration](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9d45a100-3a6d-4cb6-aa09-772ccffe9706)
and [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
provide the string-argument and regional conversion basis. This materialized
scalar implementation reuses the documented CStr profile. Full native Variant
coercion, exact error precedence, regional formatting and Access SQL qualification
remain open; it does not add a VBA execution host.

Focused expression/query tests cover numeric/fixed/date/Boolean strings, formatting
policy rebinding, grouping and correlation, NULL/mode/start boundaries, eager
failures, allocation/text limits and every observed conversion/search cancellation
checkpoint. The original [scalar search runner](testing/compare_search_scalars.py)
passes 336 authored Decimal-spelling and UTF-16 byte-matching controls, with independent raw IDs
from the unchanged four-family 20,000-row corpus. Only TableCraft executes SQL;
Jackcess supplies inert scalar getters. Existing General, LCID dispatch and strict
kernel tests retain their separate scope.


## Ordered expression-state admission (23 October 2026)

InStr/InStrRev now retain explicit states through eager SQL argument evaluation.
The shared kernel admits comparison mode first, then converts and validates start,
observes the existing NULL-string gate and finally converts haystack then needle.
Error data fails TypeMismatch when required; host Missing stays NonScalarExpression.
A NULL string can still return NULL without admitting its partner after valid
controls, as in the earlier materialized profile. Operational failures from any
written argument propagate before that gate, and volatile arguments run once.

Host Empty has an empty-Text target in string positions and numeric zero in
start/mode positions. Empty start therefore remains Domain; an Empty mode selects
binary comparison. Empty strings retain existing empty-haystack/needle rules.
Omitted reverse start still uses the final UTF-16 position. There is no SQL Empty
constructor, stored carrier or change to query Nz(NULL). Explicit CStr/CLng of
Error data remains a separate request.

The Value-only and state entry points share owned validated search options and
the existing borrowed scan/provider kernel. Raw Text limits, explicit scalar
formatting, reverse end bounds, LCID identities, consumed spans and callback/
work/cancellation failures are unchanged. No argument-list normalization copy or
provider is added. Native error precedence, full Variant coercion and regional
matching remain unqualified.

Direct query availability is established separately: Microsoft's
[select-query guide](https://support.microsoft.com/en-us/access/create-a-simple-select-query)
explicitly permits InStr in a query expression, and its
[InStrRev query examples](https://support.microsoft.com/en-us/access/functions/instrrev-function)
use SELECT over a table field. These qualify availability, not this coercion or
native locale behavior. The existing MS-VBAL/String references above supply the
argument targets and Error/Empty conversion basis.

`compare_search_replacement_states.py` adds independent binary UTF-16 models,
state/precedence controls and optional registered native-field integrations.
The scalar search/replacement runners also support source-free runs when both
`--jar` and `--directory` are omitted. Supplying either requires both; full native
modes still verify the JAR, selected corpus identities and unchanged originals.

## Explicit ICU host searches

The optional [ICU host text provider](ICU_TEXT_PROFILE.md) supplies bounded
scalar-boundary substring matching for explicit Text mode and one exact
host-registered LCID. It preserves original UTF-16 consumed widths for canonical
and expanded matches and delegates database search unchanged. The CLI activates
it with --text-locale; --text-lcid registers a numeric identity separately. Input
ceilings and cumulative work/cancellation remain explicit. The standalone-span
algorithm is not a native NLS StringSearch implementation or a completed wider
LCID/Access compatibility profile.

## Reverse callback-bound verification

The [scoped verification record](testing/results/read-query-reverse-search-bounds-2026-10-25.json)
records 40 expression tests, including five new Unicode/raw callback and finite
range-model controls. The before-fix probe retained three failures: decoded and
raw reverse searches published a position beyond their requested start, and the
finite range model found the same boundary hole. Typed callback-failure tests
already passed and remain passing. Final expression lint and the standalone
minimal build pass. No query, CLI, ICU locale or native Access execution is
claimed by this focused record.
