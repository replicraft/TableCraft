# Access application compatibility corpus

Status: acceptance policy and initial inventory, 18 October 2026. There is no
automated native application harness or qualified application baseline. The CLI
can inspect native metadata and query supported scalar tables, but cannot activate
an application. Current Rust/parser/evaluator tests do not establish
Access application compatibility.

VBA-heavy cases remain useful for preservation and dependency-detection tests.
VBA execution/authoring and COM/Wine comparisons are deferred; record those outcomes
as unsupported or unrun, never as passing because code was preserved. Required
table data-macro validation remains in scope. See [VBA decision](../VBA_COMPATIBILITY.md).

## Initial inventory

| Candidate | Provenance | Open | Preserve | Execute | Edit | Safe save/reopen |
| --- | --- | --- | --- | --- | --- | --- |
| `docs/examples/Access_Example_VBA.accdb` | Reviewed MIT example; source, notice, size and SHA-256 in [FIXTURES.md](../FIXTURES.md) | Unrun | Unrun | Unrun | Unrun | Unrun |

Read-only header/catalog inspection on a disposable copy is recorded in
[NATIVE_READING.md](../NATIVE_READING.md); the application-level outcomes above
remain unrun.

This inventory records one candidate, not a representative qualified corpus. Its
object/table metadata and a scalar row subset are inventoried, but application
object bodies, creating Access build and
required application capabilities remain unassessed.
Do not infer them from its filename or upstream description. No new application
bytes have been acquired as part of this policy.

## Corpus selection and handling

Prioritize original minimal fixtures with reproducible Access creation recipes,
then licensed representative applications: simple CRUD; VBA-heavy business logic;
linked-table frontends; complex reports/labels; custom ribbons/startup; advanced
fields. Include overlapping features, supported historical generations, signed
and compiled distribution profiles, and negative/unsupported cases.

Review provenance, redistribution rights, notices, personal data and embedded
credentials before ingestion. For private applications, obtain permission and
sanitize copies outside the public corpus; review VBA, connection strings,
attachments, histories and opaque objects as well as table rows. Record a
sanitization recipe and hash the approved derivative. Anonymization must not be
assumed from replacing a few visible names. Keep restricted evidence outside Git.

Original fixtures remain immutable; every native open, execution or mutation
experiment uses disposable copies, since Access may write metadata or lock files.
Inspect inertly first. Application execution needs explicit trust/capabilities and
an isolated, provisioned oracle environment; never run AutoExec implicitly while
collecting an inventory. Do not contact embedded linked sources automatically.

## Evidence record per application and scenario

Record these fields with each future machine-readable result, without fixing an
unimplemented harness API:

- Stable case/scenario identity, fixture hash, provenance/recipe and covered objects.
- TableCraft commit/features/target; Access and ACE exact versions/builds/bitness;
  file generation, locale/collation, references, fonts/printer, drivers and trust.
- Operation and inputs; expected native result, TableCraft result, errors/event
  traces, artifact hashes and links to approved evidence.
- Independent outcomes for identification/open, preservation, execution, editing
  and safe save followed by native Access reopen and application checks.
- Result: passed, failed, unsupported, blocked, skipped or unrun, with a reason.
  A skipped oracle comparison contributes no passing evidence. Report tested
  subsets and denominators rather than an unexplained overall percentage.

Preservation covers recognized metadata/data and opaque object bytes/streams under
the qualified edit scope. It does not require the entire edited database to be
byte-identical. Safe save requires original protection, supported constraints/data
macros, unknown-object preservation, signature policy and successful native reopen;
writing a file without an error is insufficient. Compare behavior after reopen,
not just the ability to show its object list.

## Laboratory profiles

Keep engineering oracle dependencies outside production/CLI dependency closures.
Record unavailable Windows/Access environments as skips. Add shared-file runs
across named SMB server/client implementations, Access/ACE versions, mixed native
and TableCraft clients, lock contention, interrupted writes, crash/recovery and
reconnect behavior. A local single-client success does not qualify shared use.

Forms need control/property/event matrices with cancellation and binding traces.
Reports need fonts/fallback, printer metrics, page/section limits, pagination,
grouping, labels and PDF/print comparisons. Qualify design limits by build; see
[distribution profiles](../features/ACCESS_DISTRIBUTION.md). Linked-source cases
need actual driver/server/authentication profiles, never production credentials.
P2P tests are separate from native shared-file tests and must not replay external
effects or bypass required validation.

## Next executable increment

Start with the existing candidate's verified provenance and reproducible minimal
fixture recipes. Bounded header/catalog inspection now has synthetic malformed/limit
tests and inert handling of a copy of the candidate. Its 146 metadata tuples match
an independent Jackcess comparison; object contents and native application outcomes
remain unqualified. Add a runnable Microsoft Access comparison
harness with the first operations it can actually compare, followed by small
open/preserve/edit/reopen scenarios. No placeholder crate or always-skipped test
suite is needed to make this policy appear implemented.

Use [NATIVE_ENGINE.md](../NATIVE_ENGINE.md), [VBA_COMPATIBILITY.md](../VBA_COMPATIBILITY.md)
and [WINDOWS_COMPATIBILITY.md](../WINDOWS_COMPATIBILITY.md) for the owning contracts.
Corpus failures should determine subsequent implementation priorities.
