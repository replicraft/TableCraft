# Cached Long Text calculations

Explicit stored-calculation inspection and cached row browsing now read the
observed ACE 16-family calculated Memo layout:

```sh
tablecraft inspect copy.accdb --table CalculatedMemos --stored-calculation Wide
tablecraft rows copy.accdb CalculatedMemos --stored-calculations --format tsv
```

The result is the exact cached Text, not a recomputed expression. NULL and empty
Text remain distinct, and row headers retain `[cached]`. Ordinary rows and queries
do not gain calculated-column eligibility. Inert declaration inspection and the
existing explicit row-local preview retain their separate boundaries: declarations
read no user records, and previews do not load or replace cached results.

## Storage and controls

The admitted declaration uses ACE 2016/2019-family profiles, variable updatable
MEMO storage, capacity zero, ordinary flags exactly `0x02` or `0x82` (optional
Hyperlink), exact calculated extended flags `0xc0`, matching typed ResultType and
a nonempty Expression. The Hyperlink variant retains the same bounded long-value
and cache wrapper decoder. It exposes raw Text only; no component parsing,
navigation, provider activation or RichText markup conversion occurs. Unknown
flags, widths and profiles fail even on empty tables. The reference writer persists `0xc0` and reports compression false
even when compression was requested through its builder; requested settings are
not treated as observed declaration evidence. Flag `0xc1` remains unqualified for
calculated Memo declarations.

A calculated Memo's row cell contains a native long-value definition. The reader
first resolves its checked inline, single-row or chained storage, then validates
the existing calculated-result wrapper, then decodes the enclosed Text. It does
not interpret a long-value descriptor as the calculation wrapper or strip bytes
from ordinary Memo values. The ordinary and calculated paths share the same
checked text decoding/work helper.

Encoded native limits include the wrapper; decoded Text has its independent
ceiling. Both retain their existing defaults and explicit larger-source policies.
A compressed cached payload can exceed the decoded ceiling while fitting encoded
storage; increasing only one allowance does not relax the other. Source budgets,
page/reference checks, bounded decoding work and cancellation still apply. Complete
stored inspection publishes no partial result after a late failure. Cached pages
retain their existing partial-read validation range and buffered output behavior.
No formula, provider, VBA, extraction or active content executes.

## Evidence and limits

The original [fixture and public getter oracle](../../testing/CalculatedMemoFixture.java)
creates fresh ACE 2016/2019 files with expression evaluation disabled. Read-only
getters supply exact Unicode/empty/NULL values, including multi-page caches. An
independent in-memory public `Column.read` probe checks compressed ASCII, empty
compression markers, escaped U+FEFF and supplementary Unicode inside authored
wrappers. No file is altered by these probes.

The reviewed Jackcess 5.0.4 artifact's `CalcMemoColImpl` resolves a long value before
unwrapping calculated storage; it is a physical reference, not an adopted codec.
The [comparison runner](../../testing/compare_calculated_memos.py) checks stored column
inspection, cached row output, empty declarations, ordinary-query refusal and late
wrapper corruption on disposable copies. The
[24 October 2026 report](../../testing/results/calculated-memos-2026-10-24.json) records
86 passing controls across both files, with unchanged fixed executable, oracle,
implementation and original input hashes. Microsoft Access calculated-Memo admissibility,
recalculation, freshness and native error-cache representations remain unqualified.

The [cached Text flag-family replay](../../testing/CALCULATED_TEXT_FLAG_FAMILY.md)
additionally checks public-builder calculated Memo Hyperlink flags `0x82` under
C0 alongside plain/RichText Memo and existing Short Text declarations. It records
384 passing controls and 384 cached-cell comparisons through inspect, rows and
query, with all 24 hashes unchanged. Empty declarations, NULL/empty, UTF-16 lengths,
ordinary refusal, resource limits and late corrupt-wrapper pruning/refusal remain
explicit. The engine/adapter targets pass fifteen tests with scoped strict Clippy.
Actual authored compression remains false/C0; no Memo C1 claim is added.
