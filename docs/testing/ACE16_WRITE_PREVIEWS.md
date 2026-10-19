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
It reuses the fixed non-NULL Int32 patch pipeline: expected original values,
snapshot-local ordinals, duplicate/stale assignment rejection, qualified native
constraints/properties, opaque/direct-child blockers, bounded unique Int32 index
reordering and an independent unchanged-byte proof. Field lengths, null masks,
record/page allocation and file length do not change. Other carriers, insert/delete,
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
