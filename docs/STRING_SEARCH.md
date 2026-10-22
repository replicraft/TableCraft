# InStr and InStrRev substring search

The shared text kernel implements bounded substring search in scalar evaluation
and retrieval. Positions are one-based Int32 UTF-16 code-unit offsets; no match
returns zero and a NULL string returns NULL. This counts supplementary characters
as two units, preserves embedded NUL/spaces and does not normalize Unicode.

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
not hidden by a NULL earlier operand. Non-Text string operands are not implicitly
formatted or parsed.

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
No automatic
fixed-width equality loop stands in for locale search.

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
InStrB retain their own profiles; Text mode/source coercion is not added here.

The [combined repetition/search evidence](TEXT_CONSTRUCTION.md#contextual-repetition-counts)
records 812 expression/query tests, 306 minimal-profile tests, four selected CLI
tests and 1,200 independent controls. The independent model enumerates UTF-16
windows and reverse end bounds, including empty needles and rounded sentinel/
invalid starts. Native Access comparisons remain unrun.
