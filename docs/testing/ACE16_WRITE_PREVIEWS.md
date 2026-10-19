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
constraints/properties, opaque/direct-child blockers, bounded ordinary/unique Int32 index
reordering and an independent unchanged-byte proof. Field lengths, record/page
allocation and file length do not change. Eligible mask changes are explicitly
limited to the selected bit. Indexed NULL transitions, variable carriers, insert/delete,
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
byte order. Only Int32 index mutation is enabled; other indexed targets reject,
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
wide-index edits, equivalence with indexed Int32 leaf/node/alias candidates and a
128-case generated multirow Int64 model. These are storage comparisons, not SQL
coercion, NaN semantics or actual Access behavior claims.

## Boolean and NULL-mask transitions

Ordinary non-unique Int32 key replacements now share the existing-tree pipeline;
[their contract](../NATIVE_WRITES.md#non-unique-int32-replacements) covers exact
duplicate row ties, aliases and retained unique/rule guards. This does not enable
indexed NULL transitions or foreign-index mutation.

The same fixed-update API now accepts Boolean values and transitions between NULL
and the nine qualified fixed carriers. It does not interpret Boolean false as
NULL or coerce NULL into false/zero. Boolean payload width is zero even where the
native declaration records a width of one; its logical column number selects the
mask bit. The current native reader/schema already owns that distinction.

| Transition | Allowed physical changes |
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
triggers full supported index checks after candidate construction. Indexed target
mask changes remain unsupported, including indexed no-ops; no index is silently
removed or bypassed to enable an edit.

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
Twelve rejected requests cover required/rule failures, Boolean NULL, stale/wrong
carriers, indexed targets, missing ordinals and unqualified foreign-index coverage.

Creating the enforced relationship also creates a logical type-2 foreign-key
index. Its mutation profile remains unqualified by the engine, so even a
potentially valid Children.ForeignKey edit rejects with ConstraintCoverage. This
runner explicitly checks that blocker; it does not claim successful nullable FK
mutation or native referential write qualification. Foreign-index mutation and
indexed NULL maintenance remain backend work. The fixture retains the index.
Separate [reciprocal metadata inspection](../NATIVE_INDEXES.md#reciprocal-foreign-index-links)
now checks pointer, role, action and relationship-catalog agreement without
enabling these edits.
Qualified [foreign physical reads/checks](../NATIVE_INDEXES.md#foreign-physical-traversal-and-checks)
also leave the explicit mutation gate in force.

Five CI tests cover literal bit/byte goldens, stale NULL payloads, exact no-ops,
shared-byte neighbors, field-rule rejection, late cancellation and a
128-case nullable multirow Int64 model. [Fixture hashes](../FIXTURES.md#ace-16-boolean-and-null-mask-preview-inputs)
record independent evidence separately from actual Access qualification.

[Microsoft's desktop type overview](https://support.microsoft.com/en-us/access/data-types-for-access-desktop-databases)
and [Access type synonyms](https://support.microsoft.com/en-us/access/equivalent-ansi-sql-data-types)
describe the logical Yes/No/BIT family; they are not binary row-mask specifications.
The exact transforms above are independently exercised stored-layout behavior,
not an inference from SQL Server's different BIT/null behavior.
