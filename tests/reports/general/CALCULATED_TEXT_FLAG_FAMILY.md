# Cached Text declaration families

Fresh original reference-builder/getter evidence identifies a calculated Memo
admission gap: a Hyperlink-marked calculated Memo persists ordinary flags 0x82
with the existing calculated extended flags 0xc0 and qualified wrapper. Its
read-only getter returns exact stored Text, but explicit cached inspection and
querying previously refused the declaration before decoding.

The calculated Long Text gate now admits ordinary flags exactly 0x02 or 0x82,
while retaining capacity zero, variable storage, C0, matching typed ResultType
and nonempty Expression requirements. Other calculated types still require 0x02;
Memo C1, unknown flags and fixed declarations remain unqualified. The existing bounded
long-value/wrapper/Text decoder is reused. Ordinary queries do not gain calculated
column eligibility, formula execution or freshness guarantees. See
[cached Memo storage](../access/calculations/CACHED_MEMO_CALCULATIONS.md).

## Authored independent corpus

The original [producer](CalculatedTextFamilyFixture.java) creates ACE 2016/2019
files with sixteen tables per file: eight populated Text/Memo declarations and
their empty counterparts. Short Text uses its existing ordinary flag family;
Memo covers plain, Hyperlink and typed RichText metadata. Both disabled and
requested Unicode compression are exercised. The reference writer actually
persists compression false/C0 on every declaration, even when compression was
requested; these observations do not qualify Memo C1 or broader authored flags.
The public builder refuses Hyperlink on Short Text in the excluded capability
probe, so no such declaration is added to the accepted corpus.

The [read-only oracle](CalculatedTextFamilyOracle.java) opens originals read-only,
disables expression evaluation and emits public calculation/compression/Hyperlink
getters, exact typed BYTE ResultType/TextFormat, original Expression and stored
cached getters. Producer
expression evaluation is also disabled. Every authored cache deliberately differs
from the constant stored expression, including NULL/empty, supplementary Unicode,
NUL/control characters, internal U+FEFF, larger Memo values and late unique caches.
Hyperlink strings and RichText markup remain raw Text; no URL, provider, rendering
or component conversion executes.

## Consumer and failure coverage

The [runner](compare_calculated_text_family.py) checks native declarations against
independent metadata and compares each explicit inspector, cached row page and
cached query with the same independent getter. Empty declarations are checked
through all three consumers, while ordinary calculated projections remain
refused even in empty tables. Text functions/counts retain empty-versus-NULL and
UTF-16 length behavior. An ordinary count continues to prune cached payloads.

For each populated table, a disposable copy corrupts its unique late cached
wrapper. Inspection, cached rows, TOP and restrictive cached queries must refuse
without stdout; an ordinary projection/count remains readable. Originals are
never changed. Native, query-work and output limits are explicit independent
refusals. Complete cache publication and metadata-only binding retain their
existing contracts.

Focused engine tests check raw cached Hyperlink Text, NULL/empty, explicit
metadata, the unchanged wrapper, unknown ordinary/extended flags, fixed storage,
numeric Hyperlink-flag refusal and strict Memo C1 rejection. Adapter tests check ordinary refusal,
metadata-only binding, complete-cache reuse, unrelated ordinary pruning and late
corruption under TOP/filters. These qualify the current offline engineering read
profile, not Microsoft Access admissibility, native SQL, formula behavior,
freshness, writes or coherent live reads.

## Accepted results

On 24 October 2026, the [accepted report](results/calculated-text-family-2026-10-24.json)
records 384 passing operation controls and 384 cached-cell comparisons across
ACE 2016/2019. The two files retain 32 declarations, including sixteen empty-table
counterparts and 128 authored stored cells. Every stored cell is compared through
explicit inspection, cached row browsing and cached query output. All 24 pinned
producer/oracle/runner, helper, implementation, input, reference JAR and immutable
executable hashes remain unchanged after replay.

The immutable CLI SHA-256 is
`507a7af27a386a4e37d353d095dd94d533d48656d7cc7b5f6f4d45c218f4a379`.
The engine cached-Memo target passes eight tests and the adapter cached-query
target passes seven, with scoped strict Clippy passing for both. Originals are
preserved; wrapper corruption affects sixteen disposable copies only. Native
Access calculated-Memo admissibility, formula execution, freshness, Memo C1 and
broader flag families remain unqualified. The
[fixture ledger](FIXTURES.md#original-calculated-text-flag-family-inputs) records
source ownership, original hashes and excluded capability probes.

```sh
python3 docs/testing/compare_calculated_text_family.py \
  --cli .cache/tablecraft-bin/calculated-text-family-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/calculated-text-family-20261024 \
  --report /tmp/calculated-text-family.json
```
