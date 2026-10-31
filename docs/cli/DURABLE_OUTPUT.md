# Required durable output

Normal Jet 4 `query update` and ACE engineering `query preview-create-index`
and [`query preview-before-change`](BEFORE_CHANGE_COPY.md) accept
`--durable-output`. Omission retains the existing content-sync/no-clobber
publisher. Other write-preview commands do not acquire a directory durability
claim. The verification history below records the original two consumers; the
BeforeChange guide records its separate qualification.

```sh
tablecraft query update 'UPDATE Items SET Score=27 WHERE Id=2' \
  --database offline.mdb --output edited.mdb --durable-output

tablecraft query preview-create-index 'CREATE INDEX ScoreLookup ON Items (Score)' \
  --database offline.accdb --output indexed.accdb --durable-output
```

The host validates closed statement/profile syntax before opening the source or
capturing the output. Required selection captures and holds the actual parent
directory early; it never retries a failed required operation under the default
publisher. Windows currently refuses Required at capture before staging; ordinary
Windows copies retain their existing behavior.

Planning still consumes one immutable source image. Complete report construction
and fallible source release finish before publication. The index command now
also reports source-release failure before either publication mode, correcting
its earlier silent-drop ordering. `AccessInput::into_native` recovers only its
original native resource; captured/spooled variants remain owned on refusal.
It adds no source clone, raw mutable handle or database-write permission.

The [common host publisher](../../apps/tablecraft-cli/src/native/publication.rs)
delegates Required to the [optional IO publication owner](../foundation/IO.md#explicit-required-fresh-copy-publication).
IO alone charges the requested payload bytes and owns
relative staging, file synchronization, no-clobber publication, reserved terminal
cleanup and the captured-parent barrier. The CLI consumes the success receipt
before returning its already formatted report:

    Durable output  File and captured-directory barriers completed

This acknowledges the captured directory, not continued resolution of the
original textual path. Filesystem/device compliance and physical power-loss
behavior remain separate from completed API barriers. A stdout failure can occur
after successful publication.

Errors preserve operation state. `NotPublished` means this attempt did not publish,
not that no pre-existing destination exists; a failed cleanup may leave an owned
temporary. `VisibilityUncertain` makes no absence claim. `PublishedUnconfirmed`
and `PublishedCleanupFailure` retain known visibility and barrier/cleanup facts.
These errors produce no success stdout, but the final file may exist. The CLI
never retries, deletes the final entry or claims rollback.

The Jet 4 native edit profile and all source/constraint checks remain unchanged.
ACE index output remains a non-journalable engineering copy: required filesystem
publication does not establish normal native-save permission, data-macro absence,
Access reopen compatibility or an in-place transaction.

The first macOS host source gates pass before the new publication calls:
one Jet 4 source/expected control and sixteen shared index source/expected pairs.
The original constructors remain unchanged. Shared publisher tests pass ten
functions; corrected Jet 4 tests pass five, and index command tests pass six.
The first integration run passes seven index and four normal UPDATE functions,
including sixteen Required complete index images, sixteen occupied-output
refusals, four Required encrypted/deployment refusals, and real Jet 4 default/
Required whole-image parity. Two Access input ownership tests also pass.

The first Jet 4 subset passed four tests but stopped on one test-setup parse
failure: UPDATE followed by an IN clause is outside the implemented grammar.
Replacing that one test literal with the existing `[remote].[Items]` selector
preserves the intended external-target refusal and changes no producer or
expectation. Its failed log and exact predecessor are retained. The corrected
five-function subset passes. The first strict check stopped on an IO test-only
type-complexity lint. Its alias-only correction allows final IO/Access/CLI
all-target/all-feature strict checks to pass. Full regressions pass 49 IO library,
69 Access library and 60 CLI unit functions, plus 42 existing CLI integration
functions (27 SQL UPDATE, four Memo/OLE, four preserved-Boolean and seven typed
write functions). Minimal IO and Access builds pass. No host production or source
constructor change followed these checks. Final formatting, documentation links
and dependency guards are run by the commit owner after this documentation
freeze.

These are local filesystem/API observations, not Windows, native Access or
hardware power-loss qualification. See the [host result](../testing/results/durable-output-cli-copies-2026-10-30.json)
and [IO result](../testing/results/io-required-fresh-publication-2026-10-30.json)
for scope, chronology and pinned logs.
