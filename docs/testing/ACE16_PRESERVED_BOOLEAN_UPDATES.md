# Boolean copy edits with preserved advanced fields

The existing ACE 16 engineering fixed-update service can now edit an unindexed
Boolean field while preserving qualified attachment and stored calculated fields.
The Access adapter and SQL/typed CLI copy commands use that service. It does not
enable normal saves, in-place editing or journal conversion.

The engineering snapshot exposes actual ordinary scalar values. Attachment and
calculated declarations remain unavailable to scalar assignments and expressions;
they are never replaced with NULL or silently read from stored calculation caches.
Unsupported targets and maintenance modes refuse even when no rows match.

Original-source admission checks attachment associations, calculation declarations
and wrappers, scalar constraints, exact permitted properties and current NameMap
identity. It rejects stale names, GUID aliases, unknown records and endian
disagreement. Participating relationships remain unsupported. Existing active
payload and required-macro gates remain in force.

Complete native ownership checks protect DATA from INDEX, ColumnOwned, foreign
DATA, definition and map storage, including outer attachment FileData/FileURL
storage. Following a qualified FileURL wrapper never opens its URL. The source
GLOBAL map must mark the relevant pages allocated. This nonallocating operation
receives no map rewrite authority.

Staging verifies original Boolean expectations. A separate proof receives the
original image and requests, recaptures source authority, reconstructs the allowed
mask changes and compares every candidate byte and the changed-row count. The
candidate is then requalified. No failed-write fallback is attempted.

## Observed checks and limits

Six authored rich sources pass single and batch APIs against complete one-bit
expectations, plus unchanged empty operations. Controls cover stale values,
wrong carriers and targets, NameMap anomalies, ownership/allocation contradictions,
exact resource limits and final cancellation. Actual adapter, SQL CLI and typed
CLI consumers produce six exact copies; nine refusal calls preserve the source
and leave no new output. Qualification currently covers one Boolean target per
operation, not multiple Boolean fields or general advanced-field edits.

The affected round passes 530 native mutation functions, 15 preserved-source/API
functions, 60 adapter functions and 72 CLI functions. Strict ACE/Access/CLI checks
and minimal ACE/Access builds pass. The first frontend attempt had a private
module import in test code; the first broader CLI invocation incorrectly selected
a library target. Both are retained separately from corrected passing runs.
Six strict-lint findings were corrected without changing expected images, and
the six native and two frontend API controls pass again afterward.

The donated real database still refuses with `Unsupported(Properties)`. A single
attempt on a disposable copy produced no candidate and left both source copy and
original unchanged. These authored-source results do not establish edit support
for that donated file. Native Access save/reopen qualification remains unrun.

The [result record](results/ace16-preserved-boolean-updates-2026-10-30.json)
pins the executed checks and retained donor outcome. The earlier source-only
qualification failures remain distinct from producer results; original fixture
constructors and expected images were not repaired after producer execution.

## Wider passive Datasheet carriers

The same Boolean-only profile now retains `ColumnWidth` and `ColumnOrder` with
native kind 3, flags 0 and a four-byte payload when those bytes exactly sign-extend
an existing signed 16-bit value. The original two-byte representation remains
supported. This does not reinterpret global property storage, reorder native
fields or render a Datasheet. Microsoft describes these as
[Datasheet width](https://learn.microsoft.com/en-us/previous-versions/office/developer/office-2003/aa224081(v=office.11))
and [Datasheet order](https://learn.microsoft.com/en-us/previous-versions/office/developer/office-2003/aa224067(v=office.11))
properties; their native carrier qualification is separate from those UI contracts.

A new source gate passes six pairs/twelve images before admission changes. The
API matrix passes six complete single/batch images and three exact no-ops across
the old representation, extended negative/zero values and a positive width.
Eight empty/nonempty controls refuse unqualified upper words, three-byte values,
wrong native kinds and wrong flags. These are unsupported engineering carriers,
not a claim that every such byte sequence is corrupt native storage. The final
affected runtime batch passes 540 native mutation functions, 19 table-extra
functions, 15 preserved tests plus 5 UI-carrier tests, 65 Access tests and 85 CLI
tests. Three explicit export tests remain ignored. Minimal ACE/Access builds,
formatting, documentation links and dependency boundaries pass. Final strict
ACE/Access/CLI checks also pass. The retained strict iterations include guard-only
style corrections and one iterator-idiom correction in this new source helper.
The latter happened after the first writer pass and changed no authored fixture
bytes or complete-image expectations; all five source/API tests pass afterward.

`PublishToWeb=1` remains refused. The official Access-template XML example and
upstream property declarations do not establish native desktop semantics for
that value. No donated-file retry was performed for this increment, and the
previous real-file refusal still applies. The
[UI-carrier result](results/ace16-preserved-ui-carriers-2026-10-30.json) records
this narrower evidence.
