# ACE 16 engineering write previews

Status: working opt-in physical candidate generation, 19 October 2026. Actual
Microsoft Access comparison, data-macro absence/execution and production ACE saves
remain unqualified. The user will perform Windows testing later; independent work
continues without treating that future testing as passing evidence.

## API and isolation

`tablecraft-ace/ace16-write-preview` enables
`NativeImage::preview_ace16_int32_updates`. It returns `Ace16WritePreview`, with
original/candidate bytes and changed-row count. It does not return NativeEditPlan,
has no journal/commit methods and cannot convert to a qualified plan. The feature
is disabled by default and is not enabled or consumed by the CLI/Access backend.
The normal planner still rejects every ACE file, including when preview is enabled.

The preview admits only the existing unencoded subversion-zero 2016/2019 profiles.
The shared admission helper enforces this mutation boundary even when the read
profile recognizes plain subversion-one files; read support does not qualify
preview mutation.
It reuses checked fixed-payload and row-mask paths: expected original values,
snapshot-local ordinals, duplicate/stale assignment rejection, qualified native
constraints/properties, opaque/direct-child blockers, bounded ordinary/unique integral index
reordering and an independent unchanged-byte proof. Field lengths, record placement,
allocated page sets and file length do not change. Eligible mask changes are explicitly
limited to the selected bit. Eligible indexed Byte, Int16/32/64 and Currency NULL
transitions use the page repacking profiles below. Qualified GUID, Boolean/IEEE/DATE
indexes, existing-width Decimal slots and same-width Short Text/Binary slots have
their own profiles below. Memo/OLE changes, record resizing, INSERT, schema changes,
AutoNumber assignment and broader UPDATE index topology changes remain unsupported. Ordinary-root deletion has a separate
[existing-page candidate API](ACE16_DELETE_PREVIEWS.md), with its own request and return
types; it does not turn these fixed-field APIs into a general writer.

The physical patch machinery is private and shared. Format admission now resides
at the distinct public entry points, with qualified edits and unqualified previews
kept separate by their return types. No dependency, registry feature or neutral
DB/runtime boundary changed. The native journal integration accepts qualified
plans only; this is an API distinction, not a restriction on host-owned raw I/O.

Required data macros are not detected or executed by this API. Existing blockers
can reject known opaque behavior, but passing them does not establish absence of
indirect or uninspected behavior. Candidate generation is an engineering experiment
on disposable copies, not SQL execution, a safe production save or a relaxation
of the [data-macro qualification requirements](DATA_MACROS.md).

## Runnable harness

```sh
cargo build --locked -p tablecraft-ace --features ace16-write-preview --example preview_ace16_update
target/debug/examples/preview_ace16_update original.accdb candidate.accdb Items Score 1 20 27
```

Arguments are source, fresh output, table, column, zero-based live-row ordinal,
expected Int32 and replacement Int32. Clap handles signed values and argument errors.
The example reads an explicit stable/offline source, completes the preview before
creating output, refuses existing paths, writes/syncs a new disposable file and
prints candidate count plus an explicit qualification notice. An output I/O failure
may leave an incomplete candidate; it never writes the input. It does not run SQL,
VBA, providers, startup actions or table events. The example has explicit work
ceilings and no runtime/network dependencies.

## Independent comparisons

UpdateFixture now generates original V2016/V2019 files, with expression evaluation
disabled, three LONG Id/Score rows, Required/range declarations, an Id primary
index, Unicode Memo and 12,288-byte OLE values. No native Access provenance is
claimed. [Original and candidate hashes](FIXTURES.md#ace-16-write-preview-inputs)
record the reviewed Jackcess artifact and observed results.

```sh
python3 docs/testing/compare_ace16_write_previews.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --example target/debug/examples/preview_ace16_update --ace2016 original-2016.accdb --ace2019 original-2019.accdb
```

Per family, the runner checks non-indexed Score 20→27 and indexed Id 2→27:
independent reopened rows/long-value lengths and index references, complete native
checks, equal file size and every changed byte confined to exact field/index-entry
spans. Existing candidate paths are retained. Range violations, stale expected
values and duplicate primary keys reject without output files; ordinary CLI ACE
UPDATE stays blocked. Originals remain byte-identical. Synthetic tests add
ascending/descending leaf/node cases, no-ops, format/resource/cancellation/property
gates and normal API rejection; a compile-fail doctest protects the plan boundary.

## Windows evidence still needed

Transfer the original and candidate fixtures, hashes and a record of the code
revision to the Windows test environment. Use copies of these inert fixtures;
retain originals and record exact Access edition/build, bitness and locale.
Compare original/candidate Id and Score values, primary-index order, all Memo/OLE
bytes and integrity/open errors. Native save/reopen and compact/reopen need separate
disposable copies and before/after records. Do not infer mixed-client locking or
recovery qualification from an offline open.

Positive table-event fixtures, required validation/error/rollback semantics and
qualified absence detection remain the separate next write-safety milestone.
The [data-macro plan](DATA_MACROS.md) defines those cases. A candidate opening in
Access is useful evidence, not proof that arbitrary application databases can be
written or that macros can be omitted.

## Exact fixed scalar candidates

`NativeImage::preview_ace16_fixed_updates` accepts owned `FixedScalarUpdate`
assignments with exact expected/replacement Value carriers. It now handles Byte,
Int16/32/64, Currency, Single, Double, legacy DateTime and GUID. Width-specific
private patch types preserve the original four-byte Int32 storage/budget footprint;
other widths use their own charged/fallible buffers. Field flags, declared width,
overlap checks, row masks/layout, properties, constraints and unchanged-byte proofs
remain mandatory. No encoding uses raw casts of validated semantic wrappers.

Numeric carriers do not interchange. Currency retains scaled i64 units, floats
retain raw bits (including signed zero/NaN payloads in synthetic tests), DATE retains
its validated Automation bits, and the existing uuid library supplies native GUID
byte order. Byte, Int16/32/64 and Currency index mutation is enabled; other indexed targets reject,
including empty/no-op requests. Boolean masks and eligible NULL transitions now have the separate profile below.
The [existing-width Decimal profile](#decimal-slot-and-index-write-previews) below
extends this API without enabling general variable-record edits. Other variable
fields and Extended DateTime remain future mutations, not coerced fallbacks.

```sh
cargo build --locked -p tablecraft-ace --features ace16-write-preview --example preview_ace16_fixed
target/debug/examples/preview_ace16_fixed original.accdb candidate.accdb Items Large 1 int64 -9223372036854775808 9223372036854775807
```

The explicit kind is `byte`, `int16`, `int32`, `int64`, `currency-scaled`,
`single-bits`, `double-bits`, `date-bits` or `guid`. Bits use hexadecimal without
`0x`; Currency uses exact scaled integral units. Invalid input/carrier/width fails
before output creation. Production CLI, Access backend and ordinary ACE commit
capabilities are unchanged; this is the same non-journalable preview type.

[FixedUpdateFixture](FixedUpdateFixture.java) generates fresh V2016/V2019 tables
with nine scalar fields, an unrelated Id primary key and long Memo/OLE values.
[The runner](compare_fixed_updates.py) takes `--jar`, `--cli`, `--rows`, `--example`,
`--ace2016` and `--ace2019`. It compares all eighteen independent candidates, complete
rows and long payloads, exact field spans, post-checks, wrong-carrier failure and
existing-output retention. Originals remain unchanged. [Provenance and hashes](FIXTURES.md#ace-16-exact-fixed-scalar-preview-inputs)
keep this separate from native Access qualification.

Six CI tests cover nine literal byte/carrier goldens on both families, exact no-ops,
NULL/wrong/stale/duplicate rejection, field flags, bounds/cancellation, unsupported
non-integral indexed edits, equivalence with indexed Int32 leaf/node/alias candidates and a
128-case generated multirow Int64 model. These are storage comparisons, not SQL
coercion, NaN semantics or actual Access behavior claims.

## Boolean and NULL-mask transitions

Ordinary non-unique Int32 key replacements now share the existing-tree pipeline;
[their contract](../access/writes/NATIVE_WRITES.md#non-unique-int32-replacements) covers exact
duplicate row ties, aliases and retained unique/rule guards. Indexed Int32 NULL
transitions have the separate profile below; checked foreign keys now have their own profile below.

The same fixed-update API now accepts Boolean values and transitions between NULL
and the nine qualified fixed carriers. It does not interpret Boolean false as
NULL or coerce NULL into false/zero. Boolean payload width is zero even where the
native declaration records a width of one; its logical column number selects the
mask bit. The current native reader/schema already owns that distinction.

| Transition | Allowed row changes |
| --- | --- |
| Boolean false/true | Only its own row-mask bit |
| Non-NULL to NULL | Clear its presence bit; retain ignored payload bytes |
| NULL to non-NULL | Exact fixed payload and its presence bit |
| NULL to NULL / equal Boolean | None; changed-row count is zero |

Expected NULL checks the captured presence bit, without interpreting stale bytes
under that bit. Non-NULL expectations retain exact carrier/bit comparisons. Every
other bit in a touched mask byte must remain identical. A two-span field/mask
change counts as one changed row. Required, validation, snapshot/resource/
cancellation and unknown-content checks remain mandatory. Every changed mask also
triggers full supported index checks after candidate construction. Indexed integral
and Currency mask changes can use the profiles below; other indexed target masks stay unsupported,
including their no-ops. No index is removed or bypassed to enable an edit.

The engineering harness accepts `boolean` with `true`/`false`, and the explicit
`null` token with eligible scalar kinds. Boolean `null` is rejected. For example:

```sh
target/debug/examples/preview_ace16_fixed original.accdb candidate.accdb Items Large 1 int64 null 9
target/debug/examples/preview_ace16_fixed original.accdb candidate.accdb Items B7 0 boolean true false
```

Use distinct fresh outputs for those examples. [MaskUpdateFixture](MaskUpdateFixture.java)
generates fresh V2016/V2019 files with Boolean slots across mask-byte boundaries,
nine nullable fields, required/range rules, long values, an enforced relationship
and a Boolean index. [The mask runner](compare_mask_updates.py) uses the same
arguments as the fixed runner. Per family it compares 42 candidate copies,
including fourteen exact no-ops, whole reopened rows/Memo/OLE data, exact payload/
mask spans and neighbor bits, structural/constraint checks and retained outputs.
Eleven rejected requests now cover required/rule failures, Boolean NULL, stale/wrong
carriers, unsupported indexed targets, missing ordinals and orphan rejection.
The original valid-parent NULL-to-Int64 request is no longer a rejection case:
checked foreign-index candidates have the separate profile below. Foreign indexes
remain intact, and required cascades and normal ACE saves remain unsupported.

Five CI tests cover literal bit/byte goldens, stale NULL payloads, exact no-ops,
shared-byte neighbors, field-rule rejection, late cancellation and a
128-case nullable multirow Int64 model. [Fixture hashes](FIXTURES.md#ace-16-boolean-and-null-mask-preview-inputs)
record independent evidence separately from actual Access qualification.

[Microsoft's desktop type overview](https://support.microsoft.com/en-us/access/data-types-for-access-desktop-databases)
and [Access type synonyms](https://support.microsoft.com/en-us/access/equivalent-ansi-sql-data-types)
describe the logical Yes/No/BIT family; they are not binary row-mask specifications.
The exact transforms above are independently exercised stored-layout behavior,
not an inference from SQL Server's different BIT/null behavior.

## Indexed Int32 NULL transitions

The fixed-update preview also accepts eligible Int32 index presence changes,
including exact NULL no-ops. Only changed rows create index updates; expected NULL
ignores stale payload bytes, and clearing presence retains those bytes. Index
keys use explicit nullable carriers and existing unique/required checks. The
integral extension below shares this machinery. Non-integral
indexed types, ignore-NULL indexes, unqualified foreign indexes and normal ACE writes retain
their separate blockers.

Each affected physical tree keeps its pages, per-page entry counts, heights,
child/tail/sibling links and ownership. Sorting retains key direction and ascending
row-reference ties; parent separators follow complete subtree maxima. A nullable
transition can change entry lengths. The repacker retains a usable prefix when
possible, reduces it when required, or increases compression to fit the existing
page. It rebuilds exact entry-end bits and free-byte counts. A required split
rejects the entire candidate. Fixed non-NULL updates keep their stricter original
byte-layout profile.

The only admitted index-page regions are the free-count and prefix fields, the
entry bitmap, and entry bytes through the greater of old/new used ends. All other
headers and unused trailing bytes remain identical. Candidate construction then
rechecks complete trees, row coverage, exact declarations and field rules. No-op
images remain exact; restoring a changed logical value need not restore its former
compression layout. Page allocation and file length stay fixed.

Four byte-level packing tests cover literal headers/bitmaps/payloads, compression
at capacity, valid unused single-entry prefixes, rejected splits, invalid geometry
and cancellation. Seven integration
tests cover both ACE families, leaf/node/deep trees, aliases, both directions,
unique/required/rule rejection, stale NULL payloads, exact no-ops, late cancellation,
a 128-case nullable key/row model and a 600-row fitting-versus-split batch.

The original inert [recipe](IndexedNullFixture.java) creates fresh V2016/V2019
nullable/duplicate/unique/required/rule controls and 1,000-row trees with long values.
[The runner](compare_indexed_nulls.py) accepts `--jar`, `--cli`, `--rows`, `--preview`,
`--ace2016` and `--ace2019`. Per family it matches fifteen complete candidates
(three exact no-ops), preserves original row-reference/page sets and child links,
checks native geometry and complete rows/long values, retains existing outputs
and rejects seven invalid requests without files. [Hashes](FIXTURES.md#indexed-int32-null-preview-inputs)
record independent evidence separately from actual Access/data-macro qualification.

## Bounded final-child nodes

ACE 16 previews also accept checked nodes whose final child is named by an
explicit upper-bound entry and whose tail pointer is zero. Traversal visits the
declared children only; the rewrite derives every separator from the complete
subtree maximum, including the final key/row pair. Child pointers, node heights,
entry counts, leaf links and allocation remain unchanged. Fixed non-NULL updates
retain their exact entry-layout limits; NULL transitions use the repacker above.
The Jet 4 writer retains its previous rejection of this topology.

Authored shallow/deep fixtures cover both ACE header families, both directions,
nullable/ordinary/unique keys, aliases, cross-leaf moves, explicit final-bound
updates and NULL round trips. The deterministic 128-case model includes these
topologies; late cancellation and budget failure publish no candidate. Full index
checks and byte-preservation assertions run on the resulting candidates. These
tests do not add independent-reader or Microsoft Access evidence for bounded
final-child mutation; the independent comparisons above cover their original
fixture topologies only. Production ACE saves remain blocked.

## Integral and Currency index updates

The fixed-scalar preview now shares existing-tree maintenance across Byte,
Int16/32/64 and Currency single-column ordinary/unique indexes. The column's exact
native type selects its payload/key width. Byte is unsigned; signed keys retain
their full native domain, and Currency uses scaled i64 units without conversion
through floating point. NULL keys keep their explicit presence marker. Other
numeric carriers are not interchangeable, including Int64 and Currency despite
their equal widths. [Existing index references](../access/indexes/NATIVE_INDEXES.md) still own
the read/constraint profiles.

Non-NULL replacements retain the original prefix, per-entry lengths, bitmap and
free-byte count. A changed key that breaks that layout rejects the whole
candidate. NULL transitions use the existing-page repacker, including prefix
reduction where needed. Complete subtree key/row maxima update node bounds in
both directions; duplicate keys keep ascending physical row-reference ties.
Pages, entry counts, height, child/tail/sibling pointers, row placement and file
length remain fixed. A required split still rejects the whole candidate.
Unique/required/rule, stale input, preservation, budget and cancellation checks
remain mandatory. Non-integral, ignore-NULL and unqualified foreign indexed
targets remain unsupported. The ordinary Jet 4 planner remains Int32-only, and
ACE previews still cannot become journalable plans or normal backend saves.

One codec test has independent literal width/sign/scale goldens and range/width
rejections. Five integration tests cover both families, exact extremes, adjacent
values above 2^53, duplicate keys, NULL round trips, compression, whole-image
no-ops, wrong/stale/unique rejection, late cancellation and resource failure.
Authored deep trees cover every added carrier and both bounded-final-child and
unbounded-tail nodes. A deterministic 128-case model checks nullable Int64 and
Currency keys. These authored tests are distinct from the independent fixtures.

The original inert [recipe](IntegralUpdateFixture.java) generates fresh V2016/V2019
Byte/Int16/Int64/Currency tables, ordinary/unique/required controls, ascending and
descending indexes with aliases, 1,000-row trees and long Unicode Memo/OLE values.
The [comparison runner](compare_integral_updates.py) accepts `--jar`, `--cli`,
`--rows`, `--preview`, `--ace2016` and `--ace2019`:

```sh
python3 docs/testing/compare_integral_updates.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_fixed --ace2016 original-2016.accdb --ace2019 original-2019.accdb
```

Generation and comparisons require the reviewed Jackcess artifact; no external
code or generated databases are bundled. Native Access, data macros, production
saves and broader mutation remain separately unqualified.

Observed comparisons match 56 complete candidates (eight exact no-ops) and reject
32 invalid requests without output. Both sources remain unchanged. Exact values,
all three selected index cursors, unchanged tree pages/counts/children/row-reference
sets, admitted field/mask/page spans and complete Memo/OLE values agree. The
[fixture record](FIXTURES.md#integral-and-currency-index-update-inputs) retains
source and ordered candidate-manifest hashes. These generated trees supply
independent evidence for their observed topologies; bounded-final-child mutation
still has the authored evidence described above.

## Checked foreign-index updates

ACE 16 previews now admit fixed-field changes in tables with fully checked
foreign indexes. Before planning, every selected table index must pass complete
physical/row/constraint checks. Reciprocal roles, target pointers, update/delete
declarations, primary-backed aliases and catalog key pairs must agree. When no
foreign type is visible, an explicit catalog-coverage check still rejects missing
or disguised enforced foreign declarations. This does not change ordinary read
qualification or the Jet 4 writer's foreign-index blocker.

Affected single-column Byte, Int16/32/64 and Currency trees share the existing
typed integral writer, including eligible NULL transitions, duplicate row ties,
ordinary aliases and primary/foreign aliases sharing physical data. No foreign
index is dropped, bypassed or rebuilt elsewhere. The same page/count/link/layout
and unchanged-byte requirements apply. Complete field and relationship audits
must pass before and after the whole candidate: non-NULL children need parents,
parent uniqueness and requiredness remain mandatory, and one-to-one relationships
retain dependent uniqueness. Unrelated fixed fields can change only when all
other existing table/constraint/preservation gates pass.

Reparenting to an existing key, clearing an eligible foreign key and moving an
unreferenced parent key can therefore produce candidates. Self-references and
multiple child relationships retain their complete membership checks. No cascade
or SET NULL action is executed. Parent edits leaving missing child references
fail their final membership audit, including when cascade updates are declared.
Membership alone does not prove cascade semantics: a key permutation can retain
membership while changing associations. Automatic actions remain unimplemented;
these are explicit physical candidates rather than SQL/Access update execution. Integral
composite targets now have the separate profile below. Non-integral target
indexes, allocation/splits, data macros, normal ACE saves and
native shared-file coordination remain separate work.

Five authored tests cover self-relationships, ordinary/primary/foreign aliases,
both families/directions, NULL round trips/no-ops, unreferenced parent moves,
typed orphan/unique/required/conflict rejection, malformed and disguised links,
Jet 4 isolation, cancellation/budgets and a deterministic 128-case parent-membership
model. They assert exact resulting values as well as completed relationship checks.

The original inert [recipe](ForeignUpdateFixture.java) creates V2016/V2019
Byte/Int16/Int32/Int64/Currency parents, 1,000-row dependent tables, ordinary
ascending/descending aliases, one-to-one and required controls, multiple child
relationships, declared Currency cascades, self-references and Unicode Memo/OLE
values. The [runner](compare_foreign_updates.py) compiles the repository's read-only
Java row oracle once in a temporary directory and reopens every candidate through
independent cursors. It uses the same arguments as the integral runner above,
requires a JDK with `javac`, and compares related tables, every target index,
membership/constraint reports, page/count/child/row-reference sets and exact
admitted field/mask/index spans. Native Access qualification is still pending.

## Integral composite index updates

The fixed-scalar preview now maintains two-to-ten-column indexes whose components
are Byte, Int16/32/64 or Currency. Ordinary, unique, required, primary-backed and
checked foreign declarations share one tree writer with the compact single-key
profile. Each position keeps its exact native type, width, direction and NULL
state. Updating one field replaces only that tuple component; complete key/row
pairs determine sort order and parent bounds. Clearing a component also clears
its private equality carrier, so formerly stored values cannot alter NULL tuple
equality. Stored ignored payload bytes are still retained in the database.

Composite changes always use existing-page repacking: moving partially NULL
tuples between leaves can change entry lengths or common prefixes even when the
edited field remains non-NULL. Complete page/entry counts, levels, child/tail/
sibling links, ownership, row references and file length stay fixed. A required
split rejects the entire candidate. The single-key non-NULL writer retains its
stricter original layout rules; Jet 4 composite targets remain unsupported.

Uniqueness applies to complete tuples, exempting only all-NULL keys. Partial NULL
tuples remain exact keys; required indexes reject any NULL component. Before/after
field and relationship checks still gate publication, including composite parent
membership. Automatic cascades, non-integral components,
ignore-NULL indexes, allocation, INSERT/DELETE and production ACE saves remain
unimplemented. Mixed indexes containing an unsupported component cannot be mutated.
Explicit multi-column assignments now use the batch API below.

Seven authored integration tests cover every tuple length and component position,
all integral carriers, mixed directions, aliases, exact nullable ordering, unique
swaps, all/partial NULL behavior, deep bounded/tail trees, compression, no-ops,
cancellation/budgets and a deterministic 128-case tuple model. A 600-row compressed
fixture grows to eight free bytes successfully, while a wider batch rejects the
complete candidate rather than attempting an unimplemented split.

The original inert [recipe](CompositeUpdateFixture.java) creates fresh V2016/V2019
five-column mixed/alias indexes, a 1,000-row tree, ten-column indexes, unique/
required controls, composite parents/children and Unicode Memo/OLE payloads.
The [runner](compare_composite_updates.py) takes `--jar`, `--cli`, `--rows`,
`--preview`, `--checker`, `--ace2016` and `--ace2019`. It compiles the repository's
read-only Java row oracle once, independently compares rows/cursors, and proves
unchanged page/count/child/row-reference sets and bytes outside admitted spans.

```sh
cargo build --locked -p tablecraft-ace --features ace16-write-preview --examples
python3 docs/testing/compare_composite_updates.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --rows target/debug/examples/table_data --preview target/debug/examples/preview_ace16_fixed --checker target/debug/examples/check_table --ace2016 original-2016.accdb --ace2019 original-2019.accdb
```

The read-only `check_table` engineering example uses an explicit cumulative byte
allowance (default/max 256 MiB, adjustable downward with `--bytes`), 100,000 rows
and 100 million steps. Complete index/field/relationship checks must finish before
success output. It requires a stable/offline source, changes no bytes and supplies
no shared-file or save guarantee. The CLI inspector's 16 MiB allowance is unchanged;
its full check of the 1,000-row mixed fixture exceeds that limit. The runner uses
the explicitly selected library harness for those full checks, without reducing
the fixture or hiding a failed check.

Observed independent comparisons match 36 complete candidates (four exact
no-ops), reject fourteen requests with their expected constraint/conflict errors,
and preserve originals and existing output paths. Related parent/child tables,
complete long values and permitted byte spans agree. [Recorded hashes](FIXTURES.md#integral-composite-index-update-inputs)
retain source/candidate manifests. All 21 previous non-unique single-column
candidate hashes remain unchanged across both ACE families and Jet 4 after this
generic writer refactor. These observations do not establish native Access,
required cascades, data-macro or production-save qualification.

## Explicit fixed-field batches

`NativeImage::preview_ace16_batch_updates` accepts borrowed
`FixedColumnUpdates::new(table, column, updates)` groups. All expectations and
live-row ordinals name the original captured image, including when another group
changes that row's primary key. Group identities must be unique ignoring ASCII
case; rows within a group must be distinct. The current engineering ceilings are
`NativeImage::MAX_FIXED_BATCH_GROUPS` (256) and
`NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS` (16,384 cumulative assignments), plus
the caller's shared budgets and existing format limits. These are bounded-profile
ceilings, not database-format maxima.

Every distinct requested table passes the original structural, property,
relationship and preservation gates before field collection. Heterogeneous fixed
payloads use their exact carriers. Original row-mask bytes merge only for disjoint
selected bits; overlapping payloads, repeated bits and conflicting expectations
reject the request. Each affected physical index is rewritten once with every
changed component, including multiple fields on the same row and ordinary/
foreign/primary aliases. Partial keys are not independently checked between
assignments. Complete final indexes and field/table/relationship rules gate the
whole candidate, so explicit parent and child changes can succeed together even
when either intermediate image would violate a constraint.

`changed_rows()` counts distinct changed physical rows scoped to their table,
excluding exact no-ops. Empty batches return exact image no-ops without qualifying
unrequested tables. Reordering independent groups produces the same candidate;
input is not retained. Candidate preparation is atomic in memory: any failure,
budget exhaustion or cancellation returns no preview. It does not write a source,
commit a journal, implement shared-file coordination or establish native Access
transaction semantics. This explicit entry point does not discover cascade/SET NULL
actions or execute data macros; the separate update-cascade entry point is below. Callers must explicitly name their intended changes; final membership
is not evidence that implicit native actions were reproduced. Normal ACE saves
and the CLI/Access backend remain outside this opt-in API.

Nine authored tests cover transient duplicate-key states, multiple scalar and
Boolean mask changes, scoped changed-row counts, self-relationships, multiple
tables, cross-field table rules, duplicate/stale expectations, no-ops, format/
structural limits, cumulative budgets, cancellation, order independence and a
deterministic 128-case two-component model.

The `preview_ace16_batch` example uses clap Parser and repeated six-value `--set`
specifications, grouping cells by table/column without coercion. Its exact carrier
parser is shared with the existing single-column example. It completes the batch
before creating/syncing a fresh output and refuses existing paths:

```sh
cargo build --locked -p tablecraft-ace --features ace16-write-preview --example preview_ace16_batch
target/debug/examples/preview_ace16_batch original.accdb candidate.accdb --set Parents Key 0 int64 1 2 --set Children ForeignKey 1 int64 1 2
```

This example requires an eligible stable/offline source with those exact original
values. It executes no SQL, providers, VBA, callbacks or implicit actions. A copy
I/O failure can leave an incomplete new file; the input is never overwritten.

The [batch comparison runner](compare_batch_updates.py) takes `--jar`, `--cli`,
`--rows`, `--preview`, `--checker`, `--ace2016`, `--ace2019`, `--mask2016` and
`--mask2019`. It reuses the recorded composite/mask recipes and independently
matches 22 complete batches (two exact no-ops), their 22 reverse-order controls,
and fourteen rejected requests without output. Complete related/untouched tables,
long values, exact DATE/IEEE/GUID/Currency carriers, index key/row order, page/
count/child/row-reference sets and admitted field/mask/index spans agree. Existing
outputs and original bytes are retained. [Batch hashes](FIXTURES.md#explicit-fixed-field-batch-inputs)
retain source/candidate manifests; native Access/data-macro, automatic-action and
production-save qualification remains separate.


## Declared update-cascade batches

`NativeImage::preview_ace16_cascading_updates` accepts the same original-image
`FixedColumnUpdates` inputs and returns the same non-journalable preview type.
This opt-in action planner freezes parent/child matches before changing any key.
Its current relationship profile is one-to-ten equal-typed integral/Currency/GUID
components, including exact partial NULL components; entirely NULL child keys
remain exempt. Selected reachable declarations must have known, enforced rules
and qualified scalar endpoints. Unknown or unsupported graph coverage rejects
preparation rather than disabling enforcement.

Explicit assignments pin their final values, including explicit no-ops. Each
changed parent component propagates through its original matching child rows;
identical demands coalesce, conflicting demands fail. The monotone collection of
cell demands reaches a bounded fixed point across multi-hop, overlapping and
cyclic paths. Complete active tuples are checked after propagation, including
components that did not change on the parent. This avoids false intermediate-key
validation and prevents children silently switching parents during key swaps.
Declared RESTRICT rejects changing a referenced original key even when final
membership alone would pass. Unenforced declarations do not dispatch actions.

All generated assignments retain their original expected values. Explicit and
generated cells share the existing 16,384-assignment ceiling; final explicit and
generated groups share the 256-group ceiling. Reachable scalar snapshots are
limited to 256 tables. The caller's allocation, row and work budgets and
cancellation checks cover graph reads, original-key matching, propagation and
candidate preparation. A failure returns no candidate; the existing batch writer
still owns final constraints, indexes, masks and byte-preservation proof. This
is engineering candidate preparation, not native Access SQL statement execution,
transaction durability or permission for normal ACE saves.

Update cascades and delete actions are distinct. This entry point never deletes
rows or dispatches CASCADE DELETE/SET NULL ON DELETE. VBA, callbacks, providers and
data macros remain inert/unqualified. The pinned independent reference is
[Jackcess FKEnforcer at the reviewed source revision](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/FKEnforcer.java),
whose update enforcement checks original dependent rows and distinguishes update
cascades from its delete handlers. No reference implementation code is copied.
Native Access comparison remains required; simultaneous batch key permutations
are a TableCraft engineering contract, not a claim about Access statement order.

The existing `preview_ace16_batch` example adds explicit `--cascade-updates`:

```sh
target/debug/examples/preview_ace16_batch original.accdb candidate.accdb --cascade-updates --set Roots Key 0 int32 1 9
```

Without that flag, its explicit-only behavior is unchanged. Inputs must be
eligible offline copies with the stated original values. Preparation completes
before creating a fresh output; existing outputs and inputs are never overwritten.

The original [cascade recipe](CascadeUpdateFixture.java) covers a three-level
chain, overlapping paths, mutual cycles, composite paths, RESTRICT, and separate
unique indexes with different physical sort directions. The one-to-one link
checker now admits a separately declared matching unique column set: uniqueness
does not require the foreign index to share its sort directions or physical
storage. Complete index and relationship checks still run independently.
The [comparison runner](compare_cascade_updates.py) specifies literal full
closures and compares every table, long value and index cursor with independent
readers, plus candidate equality with explicit complete batches and reversed
request order. Observed counts and immutable manifests are recorded in
[FIXTURES.md](FIXTURES.md#declared-update-cascade-inputs).


## GUID index write previews

The fixed-field, explicit-batch and update-cascade preview APIs now maintain
single GUID indexes and two-to-ten-component integral/Currency/GUID indexes on
eligible ACE 16-family copies. Ordinary/unique/primary/foreign aliases retain
the existing complete index and relationship checks. Exact NULL transitions,
whole-key swaps and explicit multi-column assignments share the original-image
expectations, row masks and final-candidate validation. The action planner also
matches GUID components exactly and can propagate declared GUID updates; entirely
NULL and partial-NULL tuples retain the documented membership policy.

Stored GUID fields use the existing little-endian UUID codec. Index keys use
canonical bytes in two eight-byte segments: the continuation tag stays unchanged
in descending order, while data and the final length tag are complemented. The
reviewed independent sources are Jackcess
[IndexData GUID/general-binary encoding](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
and the existing stored-GUID codec references in [NATIVE_INDEXES.md](../access/indexes/NATIVE_INDEXES.md).
Private physical ordering is not SQL GUID ordering or coercion.

The shared tree writer keeps compact numeric-only profiles; mixed GUID tuples
select the wider existing read-key model only when needed. Typed internal update
atoms reject numeric/GUID substitution. Single GUID keys and GUID tuples repack
existing pages, rebuilding bounds/prefixes/bitmaps within the same topology,
entry counts, child links and physical row-reference sets. Every physical index
is processed once per complete batch. Growth needing allocation/splits is refused
without a candidate. Boolean/IEEE/date indexed previews are described below. General binary, Decimal/text
indexed mutation, variable records, INSERT/DELETE and normal ACE saves remain separate.

Six authored integration tests cover both ACE families/directions, NULL changes,
uniqueness and swaps, GUIDs in every two-to-ten-component tuple position with
numeric sibling changes, full ten-GUID keys, stale/duplicate/no-op requests, budgets/cancellation and
a fixed-seed 128-case arbitrary canonical-byte model. Literal unit goldens verify
stored endian order, both segment encodings, truncation and type-domain rejection.
The [original fixture recipe](GuidUpdateFixture.java) and
[comparison runner](compare_guid_updates.py) cover a 1,000-row tree, unique keys,
GUID/composite foreign keys, declared cascades, complete key clearing and parent
permutations. Full rows, long values, independent index cursors, retained topology
and exact field/mask/index byte-span proofs are checked. A deliberately divergent
tree key requires a split and is retained as an atomic-rejection case. Observed
counts and hashes are recorded in [FIXTURES.md](FIXTURES.md#guid-index-update-inputs).
Native Access/data-macro/save qualification remains pending.


## Boolean, IEEE and legacy DATE index write previews

The opt-in fixed/batch preview APIs now maintain eligible Boolean, Single, Double
and legacy DateTime indexes, including mixed integral/Currency/GUID tuples up to
ten components. Their read profiles supply exact native order atoms; the numeric
write codec reconstructs physical bytes from those atoms without SQL coercion,
floating arithmetic or a civil calendar round trip. Single/Double retain every
bit, including negative zero, infinities and signed NaN payloads. Legacy DATE
retains exact valid serial bits, including negative aliases and sub-millisecond
fractions; invalid DATE values remain rejected by the core constructor.

Boolean requires a distinct mask rule: a clear bit is stored false and emits a
real non-NULL Boolean key. False must not enter the NULL atom path used for other
fields. Explicit Boolean NULL is rejected. Both non-NULL Boolean keys participate
in uniqueness, including tuple keys whose other components are NULL. Single,
Double and DATE keep their qualified NULL transitions. Unique swaps and multiple
fields sharing mask bytes are staged completely before final index/rule checks.

The existing compact numeric tree profile now owns all supported numeric key
carriers, while GUID-bearing tuples retain the wider profile. Boolean/IEEE/DATE
single-key changes repack existing pages; integral single-key behavior is
unchanged. All paths retain existing topology/entry counts/child and physical row
references, capacity checks, cancellation/resource budgets and unchanged-byte
proofs. Allocation/splits, Decimal/text/binary and Extended DateTime mutation,
INSERT/DELETE and normal ACE saves remain unimplemented. This change does not
expand relationship audit/action coverage to Boolean/IEEE/DATE: a table needing
unsupported relationship semantics still fails its ordinary coverage gates.

The reviewed source is the pinned Jackcess
[IndexData Boolean and floating-point descriptors](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java),
alongside the existing Boolean/IEEE/DATE read/field references in
[NATIVE_INDEXES.md](../access/indexes/NATIVE_INDEXES.md). No reference source is copied.
Six authored integration tests cover both families/directions, mask false/true,
unique Boolean swaps, all IEEE bit classes, DATE aliases/precision, NULL changes,
a full mixed ten-component key, shared masks, stale/duplicate inputs, cancellation
and all budget classes, plus a fixed-seed 128-case arbitrary Double model. Literal
unit goldens cover Boolean keys, IEEE signs/payloads, DATE serial encoding, invalid
DATEs, widths and forbidden Boolean NULL. The prior Single-target rejection test
now checks successful updates/no-ops and retains unsupported-index-flag rejection.

The [comparison runner](compare_scalar_index_updates.py) reuses the original
Boolean, floating v2 and DATE recipes. Forty-six complete candidates (six exact
no-ops), forty-six reverse-order controls and eighteen typed rejections match on
both ACE 16 families. Complete related/untouched tables, exact DATE/IEEE bits,
independent index cursors, topology/reference sets and authorized field/mask/index
spans agree. Existing outputs and immutable originals remain unchanged. Recorded
[source/candidate manifests](FIXTURES.md#boolean-ieee-and-date-index-update-inputs)
retain the independent/native distinction: these observations do not qualify
Access execution, data macros, shared-file behavior or production saves.


## Decimal slot and index write previews

The existing fixed/batch preview entry points now accept exact Decimal Values in
qualified 17-byte native variable-field slots. They require the column's declared
precision/scale, ordinary updateable flags, a unique variable ordinal, validated
row offsets and an existing full-width payload. No implicit rescaling, rounding,
floating conversion, row resizing or offset movement occurs. The custom codec
handles native sign/word order; it is not a decimal arithmetic implementation.

A present value can be replaced or cleared to NULL. Clearing changes only its
presence bit and retains the original payload. A NULL with a retained 17-byte slot
can be restored; a zero-width NULL that would need record expansion rejects with
`RowLayout`. Exact NULL-to-NULL/no-op requests retain every byte. Noncanonical
stored negative-zero aliases reject with `Conflict` rather than being normalized
silently. Unknown flags/extended layouts, stale/duplicate cells and incompatible
scale/precision remain typed failures without a candidate.

Qualified single Decimal and two-to-ten-component Decimal-bearing indexes reuse
existing-page tree maintenance in ascending/descending directions, including
eligible ordinary/unique/primary/foreign declarations and NULL transitions. The
wider exact private atom is selected for Decimal tuples; numeric-only and compact
GUID tuples retain their existing entry representations. Decimal key encoding is
separate from stored-field word order. The writer never allocates/splits pages or
changes tree topology; capacity failures reject the complete candidate. Jet 4
Decimal read/index ordering is unchanged and Decimal writes remain unsupported.

The separate cascading-batch API now includes Decimal atoms when corresponding
parent/child fields have identical native precision and scale. Original
associations, pinned explicit cells, conflict/RESTRICT rules and bounded closure
remain in force. Complete final values/indexes/relationships and unchanged-byte
proofs gate publication. This does not enable SQL Decimal arithmetic, cross-scale
relationships, delete actions, data macros, journal preparation or normal ACE saves.

The engineering harness accepts `decimal COEFFICIENT:SCALE`, including signed
coefficients, or `null`. The public carrier still validates its 28-digit envelope;
the native field can impose narrower bounds. Example after building the opt-in
`preview_ace16_fixed` harness:

```sh
target/debug/examples/preview_ace16_fixed original.accdb candidate.accdb Items Amount 0 decimal 1:4 -9007199254740993:4
```

Seven integration tests cover both families/directions, coefficient/scale extrema,
NULL clear/restore, zero-slot refusal, exact no-ops, stale/duplicate/precision
failures, unique swaps, Decimal at every position of two-to-ten-component GUID
mixed keys, budgets/cancellation and noncanonical negative zero. A fixed-seed
128-case property covers full-width coefficients. Two literal unit goldens cover
native field/index words, signs, bounds, NULL directions and legacy encode refusal.

The [original recipe](DecimalUpdateFixture.java) creates interspersed Memo/OLE and
Decimal fields, single/mixed indexes, a 1,000-row node tree and single/composite
update cascades. [Independent comparisons](compare_decimal_updates.py) match 22
complete candidates (four exact no-ops), 22 reverse-order controls and fourteen
atomic rejections across original V2016/V2019 inputs. Full table/long values,
independent index cursors, topology/reference sets and authorized field/mask/index
spans agree. Earlier 21 numeric ACE/Jet 4, 24 GUID and 46 Boolean/IEEE/DATE candidates
retain their frozen manifests. [Observed fixture hashes](FIXTURES.md#decimal-slot-and-index-update-inputs)
record original/candidate identities. Native Access comparisons, data-macro gates
and production-save qualification remain unfinished.

## Existing-width Short Text and Binary slots

`NativeImage::preview_ace16_slot_updates` accepts a table, a column and owned
`SlotScalarUpdate::new(row, expected, value)` assignments. The selected ordinary
variable field must be native Short Text or Short Binary, with the observed
writable flag `2` and qualified extension flags. Flag-zero read/DELETE fixtures
do not establish this new write admission. Values use the existing Text, Binary
and NULL carriers; exact representational expected-value checks retain case and
normalization distinctions. This is a separate non-journalable `Ace16WritePreview`
API; existing numeric/fixed entry points retain their previous gates.

The planner consumes the existing checked record-offset directory and the actual
selected slot span. It chooses a valid UTF-16 or compressed Latin-1 representation
which fits that span, then proves its exact value through the existing decoder.
The observed marker-only `FF FE` representation can retain empty Text in a
two-byte slot. An ambiguous leading U+FEFF is refused when its emitted bytes would
decode differently. Compression metadata never grants extra slot capacity.
A compressed form may fit a field whose metadata prefers uncompressed storage;
this bounded representation choice does not claim native-writer policy parity.
Binary replacements retain their exact bytes and actual width. Changing record
lengths, shifting offsets, borrowing neighboring padding or allocating pages is
unsupported.

A semantic no-op preserves the entire original image, including noncanonical
empty Text encodings. Assigning NULL changes only its presence bit and preserves
the original slot bytes. Present empty and stored NULL remain distinct, including
zero-width slots. Public-writer updates may instead repack or clear these bytes;
this preview's preservation contract does not claim identical intermediate or
canonical writer output.

Affected General-family single Text, single Binary and supported mixed tuple
indexes use their real existing row encoders. Physical aliases are maintained
once. Unchanged keys keep their checked encoded bytes; replacements are sorted
with the actual profile, then repacked within the original leaf populations.
Complete key/row maxima update ancestor separators while page identity, topology,
links, allocation and historical counters stay fixed. Capacity requiring a split
rejects the whole candidate. Memo components, unknown orders, indexed French,
ignore-NULL indexes and unqualified relationship profiles remain refused.
Unindexed Text storage does not require a collation provider.

Complete before/after index membership and native constraint checks precede
publication. Same physical width does not imply equal logical `Len`: Required,
AllowZeroLength, qualified validation expressions, uniqueness and relationship
checks still apply to final values. Unsupported rules and field properties remain
blockers even for empty requests. Macro qualification, native Access save/reopen,
ordinary backend writes and arbitrary record resizing remain unfinished.

The focused author target covers actual-width replacements, single and mixed
index keys, both admitted node forms, aliases, exact no-ops, NULL/empty residues,
stale/duplicate/growth/carrier failures, resource pools, cancellation and retry.
It reuses existing literal fixture mechanics with a separate writable-metadata
wrapper. Fresh public-builder originals/reference updates are separate evidence;
their candidate replay is not established by these synthetic tests.

[Scoped slot-update verification](results/ace16-slot-updates-native-2026-10-26.json)
records the author cases, unchanged writer/read regressions and preserved failed
attempts separately from original-file candidate comparisons.

The [bounded slot-preview consumer](SLOT_PREVIEW_CONSUMER.md) accepts a headerless
five-field quoted TSV, captures every request before opening the database, and
publishes only a complete candidate to a fresh output path.

A subsequent [original-file comparison](SLOT_UPDATE_OBSERVATIONS.md) passes its
unchanged 162-control plan: 124 full candidates and 38 typed atomic refusals.
Actual stored fields, logical index orders, complete ancestor keys and permitted
byte changes are checked independently. Public canonicalization, FEFF value
discrepancies and expressions-disabled reference writes remain separate evidence.
