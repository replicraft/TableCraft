# Exact Int64/Currency ignore-NULL index evidence

The [96-command original-data replay](results/ignored-null-wide-index-replay-2026-10-25.json)
passes with 3,924 modeled IDs, 78 successful commands and eighteen expected
refusals. There are 84 current-executable and twelve baseline controls across six
actual tables. Twenty immutable artifacts and six selected native source objects
are unchanged. This is independent engineering evidence, not native Microsoft
Access execution or compatibility qualification.

The [focused implementation checks](results/read-query-ignored-null-wide-2026-10-25.json)
separately pass 67 unique Rust tests, nine new, plus strict engine/adapter lint,
minimal adapter compilation and owned-file formatting checks. The admitted
extension reuses exact single-key codecs for Currency and ACE2016/ACE2019 Int64.
It adds no floating-point conversion, complete-table All/NULL read or native
mutation eligibility.

## Original producer/getter observations

Original Apache-2.0
[IgnoreNullWideIndexFixture.java](IgnoreNullWideIndexFixture.java), SHA-256
`077cec0801fdc52a59028c999f3e8b9a31216e5a621774bd885b0c03ec2ec15a`,
uses the reviewed Jackcess 5.0.4 JAR, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
The public builders author Currency tables in V2000/V2007/V2016/V2019 and an
additional BIG_INT table in each ACE16-family original. The producer refuses
existing destinations, disables expression evaluation and supplies a linked
resolver that refuses every target. No upstream source, fixture, imported
expression or reference SQL is copied or executed.

Each of the six actual tables contains 701 rows. Ignored ordinary keys retain
637 non-NULL entries and omit 64; the unique ignored key retains 647 and omits 54.
Each has ascending/descending keys, a logical alias sharing physical data and a
unique key. Every physical index owns five pages. Independent raw checks identify
24 logical root references as node pages of kind/version 3/1 with their actual
table owners. The alias shares the ascending root and physical data number.
Ordinary recorded unique-entry counters are 13 while actual membership is 637;
complete coverage uses independently counted eligible roots rather than treating
that counter as a live-entry count.

Public row/index-cursor getters establish stored values, NULL populations,
membership and order. BIG_INT getters must return Long. Currency getters retain
BigDecimal scale four, decimal text and exact unscaled integers; integer division
and remainder supply inverse text models without binary floating point. Pinned
reference implementation metadata getters separately provide owning table
geometry, physical numbers, counters and owned-page counts. The oracle does not
claim to use exclusively public metadata APIs.

Values include signed extrema, adjacent integers above 2^53, negative values,
zero and duplicates. Unique fields retain boundary values and repeated omitted
NULLs. Baseline inert metadata inspection locates roots for independent byte
checks; it supplies no expected query membership.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| IgnoreWide-V2000.accdb | 192,512 | `0d1abc65f3297a0724644f1ab35324d8ee37116d5195bcdb6a88da7f5df9b25f` |
| IgnoreWide-V2007.accdb | 421,888 | `9a1ef12af6df0b1b0075d9f1d0adfa9d5c62e6bf439a0604d82482ed19b43a40` |
| IgnoreWide-V2016.accdb | 552,960 | `ac37147c43e68f5e2a29dc0e839b41c9959b3b48b0801314ad954ee659c14f86` |
| IgnoreWide-V2019.accdb | 622,592 | `02bbfdbdf4172f0a527504d4ecfe73b0e78fa8937c1a3aeaf0070a05f6d81331` |

Originals, getter captures, exact producer commands and supplemental raw root
observations remain ignored under
`.cache/tablecraft-fixtures/ignore-null-wide-indexes-20261025/`. Read-only getter
and metadata operations preserve each original's complete bytes. The recipe does
not promise byte-identical files across creation times/reference versions.

## Original CLI comparison

The original [runner](compare_ignored_null_wide_indexes.py) filters captured
getter integers and exact scaled Currency units. Required reads follow the
public index-cursor order; automatic reads follow public table encounter order.
Typed endpoints cover signed extrema and 2^53 versus 2^53+1, including one-unit
Currency distinctions at magnitudes where binary floating point could lose them.
Only TableCraft executes the authored queries; reference SQL is never executed.
Every retrieval projects ID only, preserving hidden-key validation requirements.

Current full checks report all four logical indexes checked and retain 701 actual
rows for each table. Baseline checks report four unsupported profiles. Current
ascending/descending/alias/unique equality and ranges match getter membership;
automatic points/ranges preserve table order, while NULL tests decline omitted-key
indexes and scan the table. Required All/NULL reads remain refused. Baseline
required non-NULL equality also refuses. Buffered refusals have empty stdout and
the expected unsupported-query diagnostic. The accepted replay had no failed run
or corrected expected value.

Frozen current CLI:
`.cache/tablecraft-bin/read-query-distinct-wide-cli-20261025`, SHA-256
`121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c`.
Frozen baseline CLI:
`.cache/tablecraft-bin/ignored-null-int32-cli-20261025`, SHA-256
`1128c3a6e9b8a29bf667fea81d1c94809b6d7abe9566c9826e0ebd7f7bb24db7`.
The report pins six native source objects from revision
`b44494d4cd5a9350cfef1f1484148f16d5362bb7` and matching live hashes before/after.
Those selected objects do not attest an entire source inventory; the immutable
executable hash identifies the actual tested artifact. Original bytes, getters,
producer/runner/helper sources, reviewed JAR and baseline metadata captures remain
unchanged. The raw log is
`.cache/ignored-null-wide-index-replay-20261025.jsonl` with its hash in the report;
the replay command completed with exit zero during its initial tool call.

```sh
python3 docs/testing/compare_ignored_null_wide_indexes.py \
  --cli .cache/tablecraft-bin/read-query-distinct-wide-cli-20261025 \
  --cli-sha256 121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c \
  --revision b44494d4cd5a9350cfef1f1484148f16d5362bb7 \
  --report FRESH_REPORT --log FRESH_LOG
```

## Focused acceptance and boundaries

The Rust coverage includes exact extrema and neighboring keys, both physical
directions, leaf/node framing, projected hidden-key checks, empty/entirely NULL
populations, unique/required violations, missing entries and illegal NULL entries.
Typed wrong-carrier requests refuse even where Int64 and Currency widths agree.
Int64 metadata requires both qualified native scalar geometry and an explicit
ACE2016/ACE2019 header; secondary-family Int64 controls refuse.

Adapter coverage preserves automatic sequential order, explicit index order,
metadata-only binding, exact typed parameter points, snapshot retention limits,
NULL fallback, eager residual errors and corrupt-selected-index failures without
retry. Failed or successful selected snapshots never become complete table caches.
Every observed cancellation checkpoint and exhausted native resource pool remains
typed. Normal Jet 4 and ACE16 preview empty/unrelated-column mutation requests
retain the existing ignore-NULL blocker despite complete read-check coverage.

The initial new mutation test reused a consuming NativeImage across requests.
That authored compile failure is preserved in the verification record. Fresh
capture per request and exclusive branches corrected ownership; expected errors
and production behavior were unchanged. The final check counts exclude that
failed compile.

```sh
# First verify the reviewed JAR SHA256 shown above.
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/IgnoreNullWideIndexFixture.java generate FRESH_PATH V2016
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/IgnoreNullWideIndexFixture.java oracle FRESH_PATH
```

The same public recipe accepts each explicit priority profile; the two older
profiles author Currency only. No Int64 support outside ACE16, other ignored key
family, imported execution or write capability is inferred.
