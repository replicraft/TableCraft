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
It reuses checked fixed-payload and row-mask paths: expected original values,
snapshot-local ordinals, duplicate/stale assignment rejection, qualified native
constraints/properties, opaque/direct-child blockers, bounded ordinary/unique integral index
reordering and an independent unchanged-byte proof. Field lengths, record placement,
allocated page sets and file length do not change. Eligible mask changes are explicitly
limited to the selected bit. Eligible indexed Byte, Int16/32/64 and Currency NULL
transitions use the page repacking profile below. Other indexed NULL carriers, variable carriers, insert/delete,
schema changes, AutoNumber and broader index topology changes remain unsupported.

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
claimed. [Original and candidate hashes](../FIXTURES.md#ace-16-write-preview-inputs)
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
Decimal/variable fields and Extended DateTime remain future mutations, not coerced
fallbacks.

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
existing-output retention. Originals remain unchanged. [Provenance and hashes](../FIXTURES.md#ace-16-exact-fixed-scalar-preview-inputs)
keep this separate from native Access qualification.

Six CI tests cover nine literal byte/carrier goldens on both families, exact no-ops,
NULL/wrong/stale/duplicate rejection, field flags, bounds/cancellation, unsupported
non-integral indexed edits, equivalence with indexed Int32 leaf/node/alias candidates and a
128-case generated multirow Int64 model. These are storage comparisons, not SQL
coercion, NaN semantics or actual Access behavior claims.

## Boolean and NULL-mask transitions

Ordinary non-unique Int32 key replacements now share the existing-tree pipeline;
[their contract](../NATIVE_WRITES.md#non-unique-int32-replacements) covers exact
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
128-case nullable multirow Int64 model. [Fixture hashes](../FIXTURES.md#ace-16-boolean-and-null-mask-preview-inputs)
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
and rejects seven invalid requests without files. [Hashes](../FIXTURES.md#indexed-int32-null-preview-inputs)
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
their equal widths. [Existing index references](../NATIVE_INDEXES.md) still own
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
[fixture record](../FIXTURES.md#integral-and-currency-index-update-inputs) retains
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
or SET NULL action is executed. A parent edit that would need dependent changes
fails its final membership audit, including when cascade updates are declared;
this is an explicit unsupported workflow, not a partial cascade. Integral
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
membership. Required cascades, multi-column assignments, non-integral components,
ignore-NULL indexes, allocation, INSERT/DELETE and production ACE saves remain
unimplemented. Mixed indexes containing an unsupported component cannot be mutated.

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
complete long values and permitted byte spans agree. [Recorded hashes](../FIXTURES.md#integral-composite-index-update-inputs)
retain source/candidate manifests. All 21 previous non-unique single-column
candidate hashes remain unchanged across both ACE families and Jet 4 after this
generic writer refactor. These observations do not establish native Access,
required cascades, data-macro or production-save qualification.
