# Native text ordering

The current implementation identifies stored database/column sort descriptors,
encodes General legacy/General 2010 text-index payloads and reads qualified
single-column Short/Long Text indexes and text-bearing compound indexes. Single
text indexes also support explicit physical cursor intervals. A standalone
provisional full-string General weight comparer now works under explicit limits.
It does **not** yet
implement native SQL locale comparison or native
LIKE character behavior. An explicitly selected ordinal query collation
retains its existing meaning; metadata recognition never silently replaces it.

## Working metadata boundary

`tablecraft-ace::inspect_database_sort_order` reads the Jet 4 / ACE root-page
descriptor under caller budgets and cancellation. It needs no password or user
records, including for encoded databases: page zero uses the fixed header mask,
not the data-page cipher. Jet 3 and unqualified families/subversions fail.
The mask implementation is shared with native encryption rewrites; owned root
prefix buffers are zeroized and no keys/password fields enter output or tracing.

`NativeColumn::text_sort_order` returns the exact descriptor for Short/Long Text
columns only. Numeric precision/scale and complex-type metadata occupy overlapping
native bytes and must never be interpreted as text settings. Unknown/zero text
descriptors remain visible without preventing existing raw scalar decoding.

`NativeSortOrder` retains the little-endian 16-bit LCID, 8-bit sort variant and
8-bit weight-table version. The four-byte descriptor is at root offset 110 and
column descriptor offset 11 for the current 4,096-byte-page families. Known
General descriptors use LCID 1033, variant zero, version zero (General legacy)
or one (General 2010). `general_family()` recognizes those facts only. Zero
LCIDs, other locales/variants and unknown versions return no recognized General
family; no OS locale or release-dependent default is substituted.

CLI `inspect PATH` reports Text sort LCID/version/variant/family beside encryption
metadata. `inspect PATH --table NAME` adds a separate text-column descriptor
table after the existing scalar metadata table. Terminal tables and escaped TSV
use the existing presentation boundary. ACCDC contained databases use the same
APIs. Inspecting any of these fields activates no application/provider content.

## Qualification and next implementation

Original independent header bytes exercise Jet 4 and ACE codes 2/3/5/6, known and
unknown LCIDs/variants/versions, encoded headers, budgets and final cancellation.
Native schema tests keep column settings distinct from database defaults and
non-text fields. CLI goldens cover a known General legacy descriptor and the
registered example's text column. Existing row/query and encryption behavior
must remain unchanged when the shared header mask or field metadata changes.

`SortOrderOracle.java` and `compare_sort_orders.py` compare database and text-column
descriptors through read-only Jackcess with expression evaluation disabled. The
script checks unchanged source hashes. Jackcess's zero-LCID fallback is deliberately
not adopted: raw unknown descriptors stay unknown in TableCraft. This comparison
qualifies metadata extraction, not locale ordering or native Access execution.

Ten inputs agree on their database descriptor and one selected table's text-column
descriptor: the registered ACE 2007 example; `pwd-none.mdb`, `db-enc.mdb`,
`db2007-enc.accdb`, `db2013-enc.accdb`, `db2007-oldenc.accdb`, `db-nonstandard.accdb`;
and original generated `Plain.accdb`, `Binary-128-5.accdb`, `Standard-256-6.accdb`.
These exercise plain/encoded Jet 4 and all current ACE read families/codecs;
they do not constitute native authored-profile or exhaustive locale coverage.

The General text-key encoder below now uses reviewed weight-data provenance and
independent key vectors. Checked single-column Short/Long Text reads and explicit physical ranges now work;
next connect the explicit full-string model below to query comparison without
substituting truncated index keys. Do not
substitute Unicode normalization, lowercase conversion, an OS default locale or
generic ICU/UCA weights for measured Access behavior. Query comparison must keep
the backend-neutral supplied-collation contract; physical index interpretation
belongs in ACE. Preserve the original filter and fail unsupported index selections
without a sequential fallback. LIKE unit/range behavior requires its own evidence.
Other locales/variants remain explicit gaps until their behavior is implemented.

The private index walker now accepts keys without `Copy` or `Clone`: parent
separators are borrowed during recursion and selected leaf keys move into the
result buffer. Constraint checks borrow the previous key before replacing it.
This prepares traversal for variable-length text payloads without cloning them
or reserving a maximum-width buffer per entry. Existing mutation profiles retain
their explicit `Copy` requirement; text-index writes remain blocked.

## Checked Short Text index reads

`read_table_index` and `read_table_index_projection` accept single Short Text
indexes with explicitly recognized General legacy/General 2010 descriptors.
The CLI and Access adapter use the same reader with an explicit `--index TABLE INDEX`.
For the registered example:

```sh
cargo run --locked -p tablecraft-cli -- query run \
  'SELECT TAG_GRP_ID, TAG_GRP_NME FROM TAG_GRP_TB' \
  --database docs/examples/Access_Example_VBA.accdb \
  --index TAG_GRP_TB TAG_GRP_NME
```

Ascending/descending NULL markers, zero-length values and encoded payloads stay
distinct. Native page order, parent bounds, ownership, sibling chains, live row
references and complete row counts are checked. Every returned row is re-encoded
and must match its physical key, even if the text key is omitted from output.
No selected-index failure falls back to sequential reads.

Entry framing includes the value marker and caps the complete key at 510 bytes.
Longer encodings keep their first 508 bytes and replace the discarded suffix with
a two-byte polynomial digest. This is separate from the encoder's 255-UTF-16-unit
input prefix. Full-width keys need not end in a text terminator. The original
bitwise digest implementation has 24 independently recorded boundary/overflow
vectors from `KeyDigestOracle.java`; it does not use a copied reference lookup
table. Row re-encoding includes this framing before comparison.

The shared work context charges owned key buffers, byte/character scans and
digest processing to the caller, with cancellation checks before publication.
Full scans may be cached as complete snapshots; explicit range snapshots below
remain partial. No text predicate pushdown is introduced. The original SQL filter/sort and explicit ordinal
collation policy remain in force. Physical truncated keys never supply full-string
SQL equality or ordering.

Read-only checks validate eligible text indexes and exact short-key uniqueness.
A unique index containing a full-width key remains explicitly unqualified for
constraint checking: equal digests alone do not prove equal values. Unknown
LCIDs/variants/versions, Memo unique constraints and ignore-NULL indexes
remain unsupported. Ordinary native text-index writes remain blocked.

`TextIndexFixture.java` and `compare_text_indexes.py` exercise both General
families on original Jet 4 and ACE 2007/2016/2019 databases, including multi-page
ascending/descending indexes, NULL/empty values, case/trailing-space equivalence,
accents, ignored units, kana, supplementary characters and long/digested keys.
The recipe compares exact tagged rows, full/projected CLI queries, retained
filters/sorts and completed smaller-table checks, plus explicit refusals. The
General 2010 1,000-row cumulative check deliberately exceeds the fixed CLI step
budget and must publish no partial report. Sources remain unchanged. These are
independent-writer/reader comparisons, not native Microsoft Access qualification.
Registered-example regressions also verify text cursor order and reject a valid
row reference paired with another row's text key, including projected reads.

## Memo index reads

Single native Memo/Long Text indexes now use the same recognized General
profiles, row-key verification, projections and physical ranges. Short Text
still enforces its declared UTF-16 width. Memo retains the full bounded stored
value while indexing only the encoder's first 255 UTF-16 units, followed by
native trailing-space and overflow-digest rules. A prefix may separate a
surrogate pair; it is not repaired or substituted. NULL and empty Memo remain
distinct. Required key fields are fully decoded even when omitted from output;
malformed long-value definitions/chains and existing byte/work/cancellation
limits remain enforced by the owning long-value reader.

Distinct Memo suffixes can have identical keys. All Memo unique constraints
remain explicitly unqualified pending native semantics; physical prefix equality
is never used as full-value uniqueness or SQL equality. Physical ranges also
retain the existing 255-unit endpoint ceiling rather than silently trimming
arbitrary Memo bounds. This enables reading existing physical indexes, not
creating or rewriting native Memo indexes.

Microsoft's current [indexing guide](https://support.microsoft.com/en-us/access/create-and-use-an-index-to-improve-performance)
includes Long Text, while the older [DAO data-type reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/datatype-property)
excludes Memo. This documentation difference does not qualify every native
format/version for index creation. The current proof is original generated
physical indexes plus independent-reader comparisons; native Access build-specific
creation/save/reopen remains unrun.

`MemoIndexFixture.java` and `compare_memo_indexes.py` pass 648 controls across
both General versions on Jet 4 and ACE 2007/2016/2019: exact full stored Memo
rows, projected/full queries, read-only coverage, retained full-value Len/Right
expressions and physical cursor intervals in both directions. Inputs include
inline/single-page/chained values, shared prefixes with different suffixes,
spaces and supplementary characters crossing the prefix boundary. Unique Memo
checks stay unqualified. Source hashes remain unchanged.

The engineering `text_range` helper keeps its 1 MiB default output ceiling and
accepts explicit `--output-bytes` from 1 through 16 MiB. The Memo corpus requires
2 MiB for some full tagged results; the unchanged default must fail without
partial stdout. This allowance affects the helper's buffered output only, not
native input/row/work budgets or normal CLI defaults. Ordinary tests require no
Java: authored nullable-Memo metadata over fresh copies and independent prefix
bytes protect the new profile without claiming native fixture qualification.

## Physical text cursor intervals

`read_table_text_index_range` and `IndexProjectionSelection::Text` accept
`TextIndexSelection::{All, Null, Range { start, end }}`. Every variant requires a
qualified single Short/Long Text index, including All/Null/empty intervals; no wrong
carrier, missing index or unsupported descriptor becomes a sequential scan.
Bounds borrow UTF-8 strings, accept at most 255 UTF-16 units each and are encoded
under the selected column's explicit descriptor and index direction. Oversized
bounds and initial cancellation fail before reading the source. Bounds may exceed
the declared field width without being truncated to it.

Start/end refer to **physical index order**. An ascending interval can run from
Included("a") to Included("z"); a descending interval runs from Included("z") to
Included("a"). Included/excluded/unbounded endpoints are explicit. Range always
excludes NULL; Null includes no empty or ignored-character values. Reversed and
empty intervals return no rows after index eligibility is checked. Equivalent
encoded endpoints include all matching keys, including case/trailing-space
variants and any native overflow-digest collisions. This API is not a promise
of full-string SQL equality, Unicode order or Access locale predicates.

Finite endpoints are encoded once into budgeted owned keys. Conservative inclusive
parent fences retain duplicate boundary keys across row-reference separators;
exclusive endpoint filtering occurs at leaves. Skipped subtrees and unselected
records remain unchecked. Returned rows retain exact key re-encoding, projection
and ownership checks. Only All retains complete traversal/count semantics; even
an unbounded Range is a selected snapshot and must never be cached as a complete
table. The query adapter retains its existing original filters and ordinal
policy, without automatic text pushdown.

The engineering example uses clap and buffers bounded tagged output:

```sh
cargo run --locked -p tablecraft-ace --example text_range -- \
  path/to/TextIndexes-V2016-1.accdb --index TextDesc \
  --start z --end a --project-id
```

`TextRangeOracle.java` and `compare_text_ranges.py` pass 488 independent controls
on the eight original fixtures above: full/projected cursor selections, both
index directions, All/Null/open bounds, case/diacritic/ignored-unit equivalence,
exclusive/empty/reversed intervals, one-sided bounds and long/digested/supplementary
keys. Corrupt first-child copies prove descending NULL traversal skips disjoint
subtrees while complete reads reject them without output. Wrong typed profiles
fail. All source/copy hashes remain unchanged. Authored public-API tests retain
registered cursor order, projection, early UTF-16 limits/cancellation and typed
refusal; 512 generated opaque-key intervals protect conservative hull math.
Native Microsoft Access qualification and SQL locale behavior remain unrun.

## References and evidence limits

Microsoft's [Database.CollatingOrder](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao)
documents database/field-specific sort orders and their relation to database
creation/compaction locale. [LCID structure](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-lcid/63d3d639-7fd2-4afb-abbe-0d5b5551eef8)
is locale context, not a specification for native Access index weights.

Exact physical-field interpretation was checked against the already reviewed
Jackcess 5.0.4 binary and its Apache-2.0 source JAR (SHA-256
`4de1bc2f1663b0946ce39057dd90eace355f4f74b8f818c0fe167f0c65815428`),
`ColumnImpl.readSortOrder`, `SortOrder.getVariant` and `JetFormat` offsets.
The older pinned mirror does not expose the intervening variant byte; preserve
all four bytes rather than silently dropping it. The metadata implementation
introduced no upstream implementation/data. The subsequent General key encoder
adopts licensed adaptations as described below; no new registry dependency is introduced.
Native Windows/Access comparison, locale-equivalent SELECT results and general
text-index reads/seeks remain unqualified.

## General text-index payload encoding

`encode_text_index_key` accepts decoded Rust text; `encode_utf16_text_index_key`
also accepts raw native UTF-16 units, including separated surrogate units, without
replacement-character coercion. Both require a known General descriptor, caller
budgets and cancellation, and return a non-Debug `NativeTextKey`. Its bytes exclude
the column value marker and row/child pointer. Unknown LCIDs/variants/versions fail;
there is no locale fallback or implicit query-policy activation.

The physical format keeps the first 255 UTF-16 units and then discards trailing
ASCII spaces. A prefix boundary may separate a surrogate pair. These are index
key rules, **not a general SQL string equality/ordering implementation**. Do not
use this API to silently truncate arbitrary query strings or Memo values.

Both General families use their actual primary weights, character/entry counts,
secondary modifiers, unprintable offsets, expansion and kana/repeat rules. General
2010 also applies surrogate-unit weights, short-weight padding and the native
primary/tertiary overflow-tail behavior. Descending encoding inverts the proper
payload and adds its extra terminator. Fixed-size and length-derived temporary
capacities are charged before allocation, appends remain bounded, and final
cancellation prevents publication. No cache, network service or OS locale is used.

The [bundled data/adaptation record](../crates/engines/tablecraft-ace/data/general/README.md)
contains exact provenance, normalized artifact hashes, Apache-2.0 license/notices
and the deterministic engineering generator. Two adapted Rust encoder files and
the data retain Apache-2.0 obligations; other original project code remains MIT.
ACE's SPDX expression records both. Existing zerocopy supplies endian-explicit
offset reads and alignment-one borrowed weight records. Normal builds require no
Java, Python or build script and allocate no runtime weight table.

`GeneralKeysOracle.java` independently supplies 280 multi-character vectors
(560 ascending/descending keys) and 512 block digests covering every one of the
65,536 UTF-16 units in both families/directions (262,144 keys). The corpus includes
case, accents/combining marks, expansions, controls, Arabic modifier/overflow
sequences, Lao short weights, kana repeats/prolonged sounds, unpaired and non-BMP
surrogates, empty strings, trailing spaces and overlong input. A further 512
generated raw-unit sequences exercise bounded encoding. Resource/cancellation,
unknown-profile refusal and the UTF-8 wrapper also have focused tests.

This verifies an independent encoder implementation against the same reviewed
weight data. It does not independently validate those weights against native
Access, qualify text-index B-tree traversal or establish SQL/LIKE equivalence.

### Encoding measurements

On the local Apple Silicon macOS/Rust 1.99 release build, the `text_keys` benchmark
measured **about 598 nanoseconds per owned General legacy key** and **702
nanoseconds per owned General 2010 key**, over eight fixed mixed-script strings.
Allocation, budget accounting, encoding and key destruction are inside the timer;
format selection and corpus setup are outside it. Neither case measures I/O,
index traversal, SQL comparison or native Access performance.

Hyperfine 2.0.0 used two warmups and eight accepted runs per case, with 10,000
iterations of the eight-string corpus (80,000 keys per run). Complete process
times were 50.2 ± 1.6 ms and 59.8 ± 10.0 ms respectively. All accepted samples,
including slower General samples, are retained; these are different native
formats with different output/work, not optimization contenders or a speedup.
The benchmark reports raw per-run nanoseconds and a stable checksum; its raw
Hyperfine JSON and process transcript were retained for the engineering run.

Build with `cargo bench -p tablecraft-ace --bench text_keys --no-run --locked`;
run the printed executable with `0 --iterations 10000` or `1 --iterations 10000`.


## Compound text index reads

Native indexes containing Short Text or Memo now support two through ten
components, including mixed numeric/Boolean/legacy DateTime, GUID, Decimal and
Short Binary fields. Each text column retains its own General legacy/General
2010 descriptor; each component retains its own direction. Jet 4 Decimal
reconstruction preserves its legacy sign-byte behavior without enabling writes.
Fixed-only indexes retain their existing qualified profiles;
[compound Binary indexes](testing/BINARY_INDEX_TUPLES.md) now use the same owned
variable-width tuple reader.

Complete component encodings are concatenated before the whole entry is capped
at 510 bytes and digested. Truncating individual text components before joining
would produce incorrect compound keys. The reader compares opaque physical bytes
and validates every referenced live row by reconstructing the complete key,
including key fields omitted from output. Parent bounds, page ownership, sibling
chains and full row counts retain the existing walker checks. Failure never
falls back to a sequential scan. Owned buffers and byte/digest work use the
shared budget and cancellation controls.

Decoded keys can end inside a component after digesting, so they do not supply
NULL facts for constraints. Constraint checks use separately retained NULL bits
from complete decoded rows and reject attempts to use separator metadata.
Exact short-key uniqueness is checked; any Memo component or full-width key
leaves uniqueness explicitly unqualified. Ignore-NULL indexes and unknown text
sort orders remain unsupported. This adds no compound range API, SQL locale
comparison, automatic text predicate pushdown, text-index mutation or native
Access creation/save qualification.

`TextTupleFixture.java` and `compare_text_tuples.py` pass 408 independent controls
on eight Jet 4 / ACE 2007/2016/2019 General-version combinations. Original inert
inputs cover 1,000-row node-root pairs, reversed and mixed directions, ten-column
mixed-carrier tuples, different General descriptors within one tuple, NULL/empty,
case/trailing spaces, accents, supplementary characters, overflowing keys and
Memo suffixes. Complete tagged rows and two query projection shapes agree with
Jackcess; unique/empty checks complete and Memo uniqueness stays unqualified.
Corrupt selected roots fail with no stdout. Original and damaged source hashes
remain unchanged. These are independent-reader comparisons, not native Access
or SQL collation qualification. Authored Rust tests additionally check literal
mixed-direction keys, suffix-sensitive whole-entry digests, resource failures,
row-only NULL metadata and key/row disagreement with both key fields unprojected.

```sh
python3 docs/testing/compare_text_tuples.py \
  --jar path/to/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --rows target/debug/examples/table_data \
  --directory path/to/fresh-text-tuples --generate
```

The generator refuses existing output files. The reviewed JAR and generated
files remain engineering-only, local and untracked; [fixture records](FIXTURES.md#compound-general-text-index-inputs)
retain provenance, exact hashes and regeneration limits.


## Full-string SQL qualification boundary

Extending physical weight encoding beyond its native prefix does not by itself
establish SQL comparison semantics. Microsoft documents regional settings and
case-insensitive text sorting in its [sorting reference](https://support.microsoft.com/en-us/access/sort-records-on-text-numeric-or-date-values).
The comparison callback currently supplies one explicit ordering for filtering,
sorting, grouping and distinctness; per-field versus database orders and operation-
specific Long Text behavior still need native observations before automatic locale
selection or SQL text pushdown can be qualified.

Required native cases include full-value equality/ordering beyond 255 UTF-16
units, trailing spaces, case, accents, ignored/control units, supplementary pairs,
NULL/empty, mixed column descriptors and index-selected versus sequential plans.
Record operator, parameter/column type, database descriptor, engine/Windows version
and complete results; index cursor agreement alone is insufficient. The current
ordinal policy and physical index APIs retain their documented separate meanings.
The explicit CLI ordinal policy retains its meaning. The full-string model below
is separately selected and does not silently replace it or introduce a case-folding
fallback.

The [native capture recipe](testing/NATIVE_TEXT_ORACLE.md) now prepares original
Short/Long Text values and captures pair comparisons, sorting, grouping, DISTINCT,
joins, LIKE and typed-parameter outcomes through Windows ACE DAO. Its portable
checker verifies complete outcomes, lossless stored values, build metadata and
unchanged file hashes. Native execution remains unrun; capture integrity is not
SQL equivalence or native Access qualification. The portable replay tool now
executes every captured case through the CLI under explicit collation/LIKE
policies and reports typed-value/order differences, justified DISTINCT
representatives and unresolved errors. Its synthetic tests establish tooling
behavior only; native capture/replay remains unrun.

## Provisional full-string General comparison

`compare_general_text` is a working standalone engine API for explicit recognized
General legacy/General 2010 descriptors. It applies the reviewed weight model to
every UTF-16 unit, retaining supplementary pairs and all Long Text suffixes. It
trims trailing ASCII spaces, matching the current weight-model policy, without
normalizing/lowercasing or truncating to a physical index prefix. Unknown sort
descriptors fail. Comparison results, including equality, follow the model;
native SQL operation-specific differences remain to be measured later.

The implementation shares weight/secondary/kana handling with the physical
encoder but produces separate temporary comparison buffers. Internal
unprintable positions use checked 64-bit offsets, avoiding the native compact
offset's overflow/flag overlap on long values. These widened buffers never enter
native indexes, serialized records or cursor intervals. Original physical index
encoding remains unchanged. Allocation capacities use checked arithmetic;
UTF-16 scans, trimming, encoding and byte comparison consume caller-issued
allocation/work budgets and check cancellation. Both complete operands are
encoded before comparing, with no error-hiding prefix shortcut or cached subset.

Five focused tests cover case/accent/expansion examples, complete suffixes at
254/255/1,024/20,000-unit boundaries, supplementary pairs, late punctuation,
unknown descriptors, limits and every observed cancellation checkpoint. All
applicable existing independent short-key vectors agree pairwise with the
comparison model; generated Unicode triples satisfy reflexivity, reversal and
transitivity checks. These checks establish a bounded full-string weight model,
not native Access SQL semantics. Native LIKE, mixed column/database policy
resolution and safe text predicate pushdown remain
separate implementation work. Per the current user direction, Windows comparisons
are deferred and do not block this implementation work.

## General query policy

`tablecraft-access::GeneralCollation` implements the supplied backend-neutral
`Collation` contract using `compare_general_text`. Its constructor validates an
explicit sort descriptor and owns a caller-issued cumulative budget and borrowed
cancellation callback. Repeated comparisons and reused executions consume the
same budget; it is never reset per callback. Limit, cancellation and allocation
failures retain their typed expression errors; other provider failures return
payload-free `CollationFailed`. Reentrant callbacks fail rather than panicking.
The adapter infers no database/field descriptor and does not supply an Option
Compare module policy or text-mode comparer. Its database-mode search provider
uses the full-weight span model below.

The CLI exposes this adapter through read-only `query run --collation
general-legacy` and `--collation general-2010`, alongside unchanged `ordinal`.
Each run owns a separate cumulative 16 MiB comparison allocation allowance and
1,000,000 comparison steps, independent of native-read/query/output allowances.
`--native-bytes` does not raise this budget. The selected policy applies across
filtering, joins, sorting/TOP ties, grouping/HAVING, DISTINCT/UNION, IN/BETWEEN,
scalar subqueries and Min/Max. It is valid for inline, native and SQLite host
inputs; the query library imports no concrete implementation. Original predicates
are retained, and selecting a physical index does not change the SQL policy.

```sh
tablecraft query run "SELECT 'a'='A ' AS Equal" --collation general-2010
tablecraft query run "SELECT ID, Name FROM Contacts ORDER BY Name, ID" \
  --database example.accdb --collation general-2010
```

The CLI supplies its existing explicit database/default string-built-in mode for
these policies; `vbTextCompare` remains unsupported. Database/default substring
search and replacement use the explicit General span model below.
LIKE requires an explicit ANSI mode. Ordinal keeps its scalar matcher; General
now selects [UTF-16 matching](#general-utf-16-like-matching) with literal/list
expansions and a separate pattern-space policy. This remains a provisional
LIKE model, not completion of native text P0. CLI UPDATE accepts only
ordinal, rejecting General before source/destination I/O. General native SQL
qualification, complete per-field/database resolution, broader locales, native
LIKE, broader native search semantics and safe text predicate pushdown remain open.

## Stored database General policy

`query run --database PATH --collation database` explicitly selects the stored
General legacy/2010 database descriptor. No format-version or OS-locale default
is substituted. It applies the same provisional full-string comparison, search
and UTF-16 LIKE model as the explicit family policies, with unchanged cumulative
comparison limits. LIKE still needs `--like-mode`; database ANSI settings remain
separate. Omitting `--collation` does not activate this behavior.

```sh
tablecraft query run "SELECT TAG_GRP_NME FROM TAG_GRP_TB ORDER BY TAG_GRP_NME" \
  --database docs/examples/Access_Example_VBA.accdb --collation database
```

The working adapter boundary is `NativeSource::with_database_collation`, called
before any table schema discovery. It returns the source and a separately
budgeted `GeneralCollation` sharing its cancellation callback. Binding enforces
one exact descriptor on every available scalar Text field of each discovered
table, including unprojected fields. Unknown database descriptors and differing
column descriptors fail without ordinal/family fallback; unused tables are not
scanned to establish a global default. Reusing the source preserves that guard
for later bound tables. Ordinary full/projected/selected scans pass through it.
Existing explicit family policies remain caller overrides. The returned library
policy does not invent a module Option Compare setting; CLI supplies its explicit
database/default string mode as for the family flags.

This is a working homogeneous policy scope, not complete per-expression collation
resolution. Mixed database/column families, inherited expression policy and
cross-source conflicts need that later work. All collection opt-ins are currently
rejected because child text descriptors are not yet resolved. This also holds if
collections are enabled after selecting the library policy. Plain scalar access
to otherwise mixed/complex tables retains its existing limitations. Updates do
not accept the new mode; SQLite/source-free hosts require a family/ordinal flag.
Explicit `--database` with a table-free query is allowed. Saved/named retrieval
uses the same guarded source, shared policy and budgets.

Original fixture mutations exercise unknown LCIDs/variants/versions, conflicting
fields, empty-result binding, subsequent table discovery, all builder orders,
shared limits and cancellation checkpoints. A reader refusing the selected
table's user pages proves selection/binding reads metadata only and rejects
conflicts before scanning. These are portable contract tests, not native saves.

The unchanged eight-file TextIndexFixture corpus adds independent raw descriptor
reads through `TextSortOrderOracle.java`. The reviewed Jackcess oracle identifies
both database and column orders; `compare_general_queries.py` checks successful
homogeneous execution and buffered mixed-order rejection, each sequentially and
with the selected text index. Forty total query/source controls pass across Jet 4,
ACE 2007 and both ACE 16 file families. Original source hashes remain unchanged.
The observed V2019 recipe header is legacy even for its 2010-column fixture;
this illustrates why format recognition must not choose a text policy. This is
independent model/metadata evidence; native Access comparisons remain deferred.

Microsoft documents database and field collation as text sorting/comparison
metadata in [Database.CollatingOrder](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao)
and [Field.CollatingOrder](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-collatingorder-property-dao).
The conservative mismatch rejection above is TableCraft's current supported scope,
not a claim about native mixed-expression precedence.

## Source policy identity retention

`NativeSource::text_collations` now exposes one opaque host identity per available
scalar field, with identities only for Text. The adapter maps the exact native
LCID/variant/version to its host namespace, including unknown descriptors; query
code receives no native flags or concrete engine dependency. This metadata does
not activate a General/ordinal policy. Ordinary scalar projection remains inert
even when the stored text order is unsupported.

[Query preparation](QUERY_EXECUTION.md#source-text-policy-identity) captures the
complete aligned metadata under its allocation budget, retains it through field
pruning and checks it before row scans. Native full/projected snapshot schema
checks also include these identities. Changed/unknown metadata is kept separate
from supported comparison behavior. The existing homogeneous database guard stays
in force; [bound expression lineage](QUERY_EXECUTION.md#bound-expression-text-policy-lineage)
now retains the contributing identities, while per-operation comparer routing,
complete native precedence and safe text pushdown still need implementation.

## General substring spans

The [LIKE element dictionary](#general-like-element-dictionary) is a separate
derived lookup for class/range work, not substring matching or native indexing.

The standalone `find_general_text` API accepts `GeneralTextSearch::ForwardFrom`
or `ReverseThrough` zero-based UTF-16 bounds and returns an owned consumed range.
The Access adapter supplies it through `Collation::find` for database comparison,
sharing the same cumulative budget/cancellation as ordinary comparison. CLI
InStr database/default modes, explicitly selected database InStrRev, and database
Replace now use it. Binary defaults for InStrRev/Replace are unchanged; text-mode
and explicit LCID search remain separate gaps.

The model selects the earliest/latest valid Unicode start in the requested
window, then the shortest nonempty substring with an equal complete General
weight key. This handles unequal-length spans such as `ß`/`ss` and composed/
decomposed accents without searching replacement text again. UTF-8 boundaries
are never split; returned positions count UTF-16 units. A bound inside a
supplementary pair rounds toward the searched region. An actually empty needle
uses an empty boundary span; nonempty ignorable needles must consume text, so
unlimited replacement always advances. Full-string trailing-space equivalence
applies to candidate spans too: `a ` can match/consume just `a`. These are explicit
model rules, not assertions about native Access substring tie-breaking.

The legacy weights ignore the tested supplementary emoji while General 2010
retains its primary weights. An earliest legacy match can therefore include
that ignored prefix; the two profiles are not forced into a generic Unicode
normalization/case-folding policy. Native SQL/search differences remain deferred
for later Windows comparisons under the user's current direction.

Needle encoding happens once per provider call. Budgeted boundary tables retain
UTF-8/UTF-16 offsets and conservative primary-byte bounds for remaining suffixes.
Static inline widths are exact; contextual repeats use their maximum width.
Provably insufficient suffixes are skipped. Exponential endpoint probes and a
lower-bound search locate the first candidate with enough primary bytes; an
append-only primary-prefix mismatch prevents futile extension. Complete keys
still decide equality. All allocations, scans, probes, comparisons and final
publication check shared limits/cancellation. Generated tests compare this
pruned path with exhaustive nonempty substring enumeration, including kana,
repeats, accents, ignored units, expansions and supplementary boundaries.

`GeneralSearchOracle.java` and `compare_general_search.py` independently enumerate
short complete-key spans through the reviewed Jackcess encoder. The release CLI
passes 268 forward/reverse/replacement controls across both General families.
This validates the chosen bounded weight/span model, not native SQL. Java is
compiled to a temporary directory so the package-private reference encoder uses
the same class loader; no database, compiled class or external implementation
is copied into Git. Existing artifact provenance/license records apply.

The `general_search` standalone benchmark checks expected results before timing,
including temporary allocations/destruction and work accounting. It covers
expansion lookup plus 1,025-unit common-prefix match/miss cases. For example:

```sh
cargo bench -p tablecraft-ace --bench general_search -- 1 long-match --iterations 100
```

An engineering Hyperfine run measures all six family/workload combinations with
five process runs and one warm-up, retaining raw JSON samples. Process wall time
includes launch/scheduling overhead; the harness separately prints nanoseconds
per search. These are different workloads, not competing implementations or a
claimed speedup. Difficult low-primary-weight/common-prefix cases can still
exhaust the cumulative allowance and must fail without partial query output.

## General LIKE element dictionary

Microsoft's [LIKE documentation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/like-operator)
describes language-dependent equivalence between a special character and its
multi-character sequence, including when the special character occurs in a list
or range. The [Access SQL reference](https://support.microsoft.com/en-us/access/like-operator)
also defines character lists/ranges and wildcard counts. Those contracts motivate
a distinction between collation elements and arbitrary strings whose ordering
happens to fall between two endpoints; they do not qualify our General model.

`is_general_like_unit` now supplies a working standalone recognizer: a complete
raw UTF-16 span is accepted exactly when its General weight key equals some
single UTF-16 unit's key. The immutable derived set therefore recognizes `ss`
and `ae` through their single-unit equivalents, but not arbitrary words. It
retains trailing spaces, isolated surrogate units and ignorable-unit behavior,
unlike full SQL comparison's trailing-space policy. Empty/ignorable spans can
equal a single ignored unit; actual token consumption belongs to the matcher.
Unknown locale/variant/version descriptors fail without an ordinal fallback.

The selected `fst` library reads two compiled sets totaling 109,185 bytes:
33,231 bytes/35,886 distinct keys for legacy and 75,954 bytes/56,356 keys for
General 2010. There is no input-dependent construction/cache, external FST file
read or native index substitution. Key encoding and complete lookup bounds are
charged to the caller before traversal; cancellation is checked before publication.
Existing [weight provenance](../crates/engines/tablecraft-ace/data/general/README.md)
and notices apply to these original derived artifacts. Their CRCs/SHA-256 values,
all 65,536 unit keys and exact counts are checked; the regeneration tool
reproduces both byte-for-byte without updating hash expectations automatically.

The `general_like_units` harness validates identical member/nonmember probes
before comparing FST, BTreeSet and sorted-vector lookups. Construction is outside
the intra-loop lookup timer. Complete classification is a separate mode that
includes key allocation/destruction and work checks. A local sample reported:

| Family | FST lookup ns | BTreeSet lookup ns | Sorted lookup ns | Complete classification ns |
| --- | ---: | ---: | ---: | ---: |
| Legacy | 35.0 | 76.6 | 138.2 | 529.9 |
| General 2010 | 45.5 | 84.6 | 147.3 | 536.3 |

These are nanoseconds: complete classification here was about **0.53 microseconds**,
not milliseconds. Samples are hot standalone operations, not full query latency
or confidence intervals. Hyperfine retains five process runs and one warm-up
for all eight family/mode combinations; process wall time includes alternative
construction and launch/scheduling, so it must not be presented as pure lookup
latency. Workloads span the dictionary and include nearby missing keys.

```sh
cargo bench -p tablecraft-ace --bench general_like_units -- 1 fst --iterations 1000
cargo bench -p tablecraft-ace --bench general_like_units -- 1 classifier --iterations 1000
python3 scripts/regenerate_general_like.py
```

This component now supplies element recognition to the explicit UTF-16 matcher
below. The dictionary alone does not establish pattern semantics or native
qualification.
Pattern parsing and transitions remain in the expression layer through supplied
policies; this engine API interprets weights only, never wildcard syntax.

## General UTF-16 LIKE matching

The expression layer now owns an iterative matcher for explicitly selected
`LikeProfile::Utf16Weights`. General adapter/CLI policies opt in; ordinary
collations default to the unchanged Unicode-scalar profile. Wildcards consume
UTF-16 units independently of weights, while literal runs compare complete raw
spans and lists/ranges require single-element equivalence through the derived
FST. The engine's `compare_general_like_units` retains trailing spaces and raw
surrogate units; SQL comparison/search trimming is not reused for pattern literals.

```sh
tablecraft query run "SELECT '😀' LIKE '??' AS Units, 'ss' LIKE '[ß]' AS Expanded" \
  --collation general-2010 --like-mode ansi89
```

Contiguous literals, including those joined by empty `[]`, admit equivalent
multi-unit spans. Lists admit one element's complete span, preventing arbitrary
words from matching a range simply because their keys sort between endpoints.
Negated lists use excluded-prefix precedence: any excluded equivalent element
prefix at the current start prevents shorter nonexcluded alternatives from
bypassing it. Token boundaries are explicit; no partial weight stream is split
across a literal/list/wildcard boundary. These model choices need later native
observations, especially supplementary characters, ignored units, contractions
and negative/list tie-breaking. Other locales and automatic field policies remain
separate work; Windows comparisons stay deferred at the user's direction.

Both ANSI modes retain explicit syntax, range validation, ASCII digit rules,
literal spaces and NULL/error ordering. All buffers/transitions and provider
normalization share their owning budgets and cancellation contracts. Matching
uses three reusable position buffers, no recursive backtracking, and checks
before publishing truth. A final literal compares complete tails directly;
other variable-span transitions stop only on a provider proof that no extension
at the same start can match. Literal pruning checks the append-only primary
weight prefix against the complete target key. Element pruning traverses the
immutable single-unit FST directly, without allocating a search stream. Host
defaults never prune. Generated extension controls check both proofs against
complete comparisons/recognition, including ignored units and raw surrogates.
An ordinary 128-unit `*a*` contains control now succeeds within unchanged budgets;
the initial unpruned implementation exhausted its allocation allowance. Complex
inputs may still exceed current ceilings without partial output.

An identity-provider property test compares the new path with the existing
scalar matcher on generated ASCII inputs/patterns. Focused cases exercise
expansions, empty groups, raw unit counts, pattern spaces, negation, bad patterns,
shared budgets, correlation/grouping and every observed cancellation point.
The independent engineering recipe `compare_general_like.py` passes 1,792
short pattern controls using the reviewed Jackcess binary/source artifacts. It
preserves original licensed source only in a temporary directory and replaces
the index-only string preparation method with a raw-literal adapter; the weight
encoder remains independent. The modified source SHA-256 is
`4f7822cc89d43f8e96fbd128b1fdd2c7bd768dbd1961151609a3ac14e91a7ceb`.
A recursive reference matcher, explicit pattern models and an independently
enumerated single-unit set supply results for both families/modes. This proves
agreement with the chosen model, not native Access SQL. Original JAR hashes
remain unchanged; no reference source/class/database is tracked.

The `tablecraft-access` `general_like` harness validates each expected result
before timing complete standalone LIKE calls, including fresh policy/budgets,
pattern compilation, matching, temporary allocations and destruction. Input
construction, query planning, file I/O and output formatting are excluded. One
local intra-loop sample per family/workload reported microseconds per call:

| Family | Literal expansion | Range element | Negated list | Two-unit wildcard | 1,025-unit literal | Contains, 128 units |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Legacy | 1.515 | 3.787 | 6.221 | 0.470 | 41.577 | 444.224 |
| General 2010 | 1.514 | 3.822 | 8.001 | 0.468 | 43.794 | 539.807 |

These are microseconds, not milliseconds or full-query timings. Twelve family/
workload combinations also ran through Hyperfine with five process samples and
one warm-up, retaining raw results. Process wall time includes launch/scheduling
and is not the per-call timer above. No cross-profile speedup or confidence
interval is claimed; unlike semantics must not be treated as competing results.

```sh
cargo bench -p tablecraft-access --bench general_like -- 1 class --iterations 100
```
