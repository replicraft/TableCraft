# Project architecture qualification

Acceptance plan, not passing evidence or installed runtime test gates. Current
headless/native-reader results remain in [IMPLEMENTATION.md](../IMPLEMENTATION.md).
Architectural completeness percentages/ratings from proposals are not release metrics.

| Proof | Required evidence before claiming support |
| --- | --- |
| Project evolution | Create older versions, migrate deterministic multi-step paths, preserve definitions/data, reject unsupported future/required features and recover after interruption |
| Packed/unpacked equivalence | Stable IDs, source bytes, versions, resources and semantics survive round trips; stale workspace updates conflict; traversal/duplicate/oversize resources fail |
| Native ACE 16 interoperability | Modify actual ACCDB, preserve unrelated/opaque/VBA content, reopen in recorded Microsoft Access build and verify values, indexes and application objects |
| Mixed-client transactions | Access/TableCraft simultaneous writers, contention, cancellation, native scope, crash/recovery on each qualified OS/filesystem/SMB configuration |
| Native application execution | SQLite data, form, Python validation, report and canonical command lifecycle work after pack/open on another qualified OS; worker failure and permission denial recover |
| Access application conversion | Representative data/queries/forms/reports/macros preserve verified workflows, with every unresolved VBA/external dependency reported and retained |
| Peer evolution | Deterministic convergence, old-epoch offline changes, incompatible peers, executable trust and interrupted upgrades preserve history and block unsafe apply |
| Host parity | CLI/desktop/web supported operations obey identical compatibility/authorization decisions; visual behavior separately qualified per platform |

Each feature profile must produce machine-readable evidence: implementation
revision, fixture source/license/hash, format/build/schema versions, operation,
host OS/architecture, filesystem/network topology where relevant, feature flags,
expected/observed results and known limits. Mark passed/failed/skipped/unrun
explicitly and distinguish synthetic, independent-reader and actual Access
oracles. Existing benchmark JSON is not project migration/interop proof.

Future release jobs must gate the claims they publish on migration history,
unsupported-object preservation, mixed-client recovery, deterministic replication,
Python lifecycle/permissions/packaging and host parity evidence. Unavailable Access
or browser environments must remain unqualified/skipped; no successful headless
job fills that gap. Introduce each test gate with runnable feature functionality,
not empty jobs that print success.

Record representative startup, peak memory, query latency, large-table scrolling,
migration duration and replication overhead budgets with the first proofs. Reject
unbounded eager loading as the default project-open design. Numeric thresholds
require measured workloads, supported machines and baseline evidence.

Near-term implementation remains ACE 16 advanced fields/write safety. A later
small `.tcraft` identity/version/backup/migration proof must precede full Python
and designer implementation; it is not permission to bypass the current SQLite
pause or create empty packages now. Engineering dependency-boundary checks are
separate from these future behavioral proofs.
