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
