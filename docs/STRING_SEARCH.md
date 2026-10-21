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

Supplied start/mode values use checked half-even numeric-to-Int32 conversion.
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
The [-1/0/1/2 mode conversion contract](STRING_COMPARISON.md) is shared. LCID-valued
InStr comparison arguments are not implemented.

`Collation::find` is a separate search callback. Merely supplying equality/order
comparison does not implement locale substring matching. Its `StringSearch`
boundary is either a zero-based UTF-16 forward start or exclusive reverse end.
Providers return a UTF-16 match range, allowing expansions/contractions where
source and needle lengths differ. Returned bounds are validated before emitting
a position; invalid ranges return InvalidTextMatch. Providers own callback work,
cancellation, match correctness and earliest/latest selection. No automatic
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
shared cumulative budgets and module/default contracts remain explicit. Text-mode
and LCID search are still unsupported; the chosen model is not native SQL
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
LCID modes and lossless unpaired-surrogate inputs remain open. This is `Partial`
in the [built-in tracker](ACCESS_BUILTINS.md), not full native Access qualification.

[Replace](TEXT_REPLACEMENT.md) also consumes the search provider, advancing through
non-overlapping suffix spans and requiring nonempty consumed matches for progress.
It charges construction separately through a bounded output sink.
