# Explicit stored Memo history queries

`query run --database COPY.accdb --collections` exposes qualified stored Memo
histories as aligned named collection members. This is a TableCraft engineering
view over stored records, not a claim that Microsoft Access accepts the same SQL
member expressions. Native query comparisons remain unrun.

Discover the actual hidden history field and its conceptual/flat table names
through `tablecraft inspect COPY.accdb --table NAME --complex`. Inspect those
named tables with `inspect --table TABLE` to see their member declarations.
Do not construct a hidden or modified-time field name from the visible Memo
name. The conceptual history value-field name identifies the visible current-value
Memo; its modified-time field and matching flat fields are also resolved from
metadata. For example, if inspection reports hidden `Notes$History` and its
template/flat declarations contain `Notes` and `Modified`:

```sh
tablecraft query run 'SELECT ID, Notes AS CurrentMemo, [Notes$History].Notes AS HistoryText, [Notes$History].Modified AS HistoryTime FROM Histories' --database COPY.accdb --collections
tablecraft query run 'SELECT ID, Count([Notes$History].Notes) AS N FROM Histories GROUP BY ID ORDER BY ID' --database COPY.accdb --collections
```

These names are examples, not generated-name rules. The original synthetic Rust
fixture deliberately uses hidden `Score`, visible/member `Notes` and `Modified`
to catch accidental name construction. Another original reference fixture uses
visible/member `Details` in a separate table.

## Stored-record contract

The checked ACE 2007 and ACE 16-family profiles expose exactly the actual stored
Text and legacy DateTime members of each qualified hidden history field. The
visible Memo continues to read its current stored scalar value. History queries
do not replace that value, reconcile it against historical entries, choose a
latest version, infer chronological ordering or run any expression or handler.
Native Access history updates and writes remain separate work.

The visible current Memo admits its ordinary flag family: exact flags 0x02
(updatable) or 0x82 (updatable plus Hyperlink), and exact extended flags 0 or 1
(Unicode compression). It retains the existing Memo decoder. Hyperlink component
strings and RichText markup/properties remain raw data: no parsing, navigation,
provider activation or markup conversion occurs. This does not change historical
child encoding or infer a newest value. Unknown/calculated visible flags,
unsupported ordinary geometry and nonzero template/flat extended flags remain
refused. Child/template ordinary flags retain their existing gates. Metadata-only
discovery and complete child validation still apply.

Within one history, members remain aligned to the same stored child. Flat stored
order, duplicate values, Unicode/NUL text, NULL/empty Text, nullable timestamps and
exact DATE carriers survive the borrowed source view. SQL projection uses the
existing formatter; it does not expose raw DATE bits as decimal numbers. Empty
histories retain the existing explicit collection NULL-extension policy. Distinct
collections use the existing bounded product policy. These policies are explicit
TableCraft behavior, not native qualification.

Displayed member criteria filter aligned children. Unprojected member criteria
use the existing parent-membership rules. Bare hidden-field `IS NULL` / `IS NOT
NULL` tests complete collection presence, independently of nullable child cells.
An all-NULL child still establishes presence. Bare hidden-field projection and
FileData on a history are unsupported. Default and `--multivalues` modes do not
publish history members. Enabling decoded attachment views does not add FileData
to histories or decode their Text.

## Metadata, resources and caches

`inspect_version_history_declarations` validates metadata without reading parent
keys, child records or Memo payloads. It requires the existing typed history
template, visible Memo association, hidden-key marker and matching qualified flat
fields. Corrupt resolved mappings fail; unsupported candidates remain unavailable.
Discovery declines an entire
unsupported candidate, including unsupported visible-Memo or flat-field flags;
it does not block unrelated qualified history/attachment members. The hidden
field remains in unavailable-column metadata, and explicitly querying its absent
members or presence still fails rather than returning an empty history. Corrupt,
cancelled and resource-exhausted discovery is never downgraded to unsupported.
Each Text member retains its
actual flat-field sort descriptor. Templates and database defaults do not replace
unknown descriptors; metadata discovery never activates a comparison provider.

Execution loads a complete checked history per referenced hidden field, using
the existing snapshot, native work/allocation and cancellation controls. Value
and modified-time member references borrow that same snapshot; they neither copy
Text to a second query cache nor reread child rows for each selected member.
Partial/failed reads are never published as complete caches. TOP and restrictive
filters do not hide corruption in a selected complete history. Unreferenced
history records remain unloaded, so ordinary scalar counts do not depend on their
integrity. Explicit indexes on a collection-reading parent remain rejected with
no silent sequential fallback. Existing native, query, Text and output controls
continue to apply.

## Verification

The [compressed current-Memo replay](../../testing/COMPRESSED_MEMO_HISTORIES.md)
adds 198 independent controls across original ACE 2007/2016/2019 files: six
metadata comparisons and 96 stored-query controls under each of ordinal and
database policies. All 26 pinned hashes remain unchanged. Current/child values,
association names, NULL/empty histories, grouping, member criteria and refusals
are compared without native Access execution. The expanded engine/adapter
batch passes 19 tests, including calculated/unknown flag and late-orphan controls.

The [ordinary visible-Memo family replay](../../testing/MEMO_HISTORY_FLAG_FAMILY.md)
further checks all four ordinary/Hyperlink and uncompressed/compressed combinations,
plus original RichText metadata and raw markup. It passes 594 controls over nine
fresh ACE 2007/2016/2019 files, with all 32 pinned hashes unchanged. The current
engine/adapter history targets pass 22 tests with scoped strict Clippy. Historical
child/template flags and codecs remain unchanged; no hyperlink navigation or rich
text conversion is implied.

`tablecraft-access/tests/version_history_source.rs` covers metadata-only binding,
all three qualified profile codes, current/history separation, paired member
alignment, stored order, NULL/empty members, DATE bits, collection presence,
aggregates, residual filtering, complete-cache reuse, explicit modes, selected
index refusal, corrupt references, limits, execution cancellation and flat Text
policy routing. The engine's original history fixture is shared by these tests
and its existing typed-reader tests. A mixed two-history control separately
checks unsupported visible-Memo and flat-member candidates without losing the
supported history or fabricating unavailable members.

[The independent runner](../../testing/compare_version_history_queries.py) uses the
original [history recipe](../../testing/VersionHistoryFixture.java), its unchanged
[stored-record oracle](../../testing/VersionHistoryOracle.java) and a new read-only
[declaration/current-value oracle](../../testing/VersionHistoryQueryOracle.java).
It compares stored order, alignment, NULL-extension, current values, aggregates,
presence, member filtering, mode/member refusals and resource failures, checking
the CLI binary and original input hashes after replay. These are independent
stored-record/model comparisons; Microsoft Access SQL, newest-first semantics,
mutation and application save/reopen are unqualified.

The 2026-10-24 [recorded replay](../../testing/results/stored-history-queries-2026-10-24.json)
passes 96 controls with explicit Ordinal comparison and another 96 with database
policies across three fresh original ACE 2007/2016/2019 inputs. The CLI binary
and all original input hashes remain unchanged.

```sh
python3 docs/testing/compare_version_history_queries.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --ace2007 history-2007.accdb --ace2016 history-2016.accdb --ace2019 history-2019.accdb
```
