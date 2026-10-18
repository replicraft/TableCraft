# Transaction ownership

Accepted boundaries; general begin/commit/rollback contracts are not implemented.
Current [DB snapshots/edit planning](../DB.md),
[offline native reads](../NATIVE_TRANSACTIONS.md) and optional
[image journals](../IMAGE_JOURNAL.md) retain their documented limits. The image
journal is not evidence of native Access recovery or mixed-client safety.

| Owner | Responsibility |
| --- | --- |
| `tablecraft-db` | Future shared modes/options, supported guarantees, cancellation/contention, transaction scope, commit/rollback outcomes and capability discovery |
| `tablecraft-ace` | Native page/index/allocation mutation, locks, cache coherence, commit/rollback and qualified native recovery; standalone API independent of DB/runtime |
| `tablecraft-access` | Translate native engine operations/capabilities into DB contracts |
| `tablecraft-sqlite` | Delegate connection transactions/savepoints and errors to SQLite/rusqlite; no replacement physical transaction manager |
| `tablecraft-sync` | Commit local projections and outbound replication metadata together where feasible; validated remote apply without user-event replay |
| `tablecraft-runtime` | Canonical command lifecycle, explicit connection scope and post-commit effects |

Native Access workspace transaction scope is not inferred from a form or one
connection. DAO documents workspace-level operations and qualified nesting, with
different limitations for ODBC. SQLite connection transactions and savepoints are
different capabilities. Expose supported scope/isolation/nesting explicitly;
never advertise savepoints or an isolation guarantee a backend cannot supply.
Native temp-log/recovery details need investigation and native comparison, not
assumed SQLite-style WAL or copied distributed storage algorithms.

Finalize ownership/lifetimes when a working transactional consumer exists. Shared
contracts must distinguish committed, rolled back/aborted and uncertain outcomes;
an I/O error is not proof of rollback. Commit is consuming/terminal under the
eventual contract, and failure recovery must prevent accidental second submission.
Cancellation after an uncertain commit requires reconciliation, not blind retry.
Lock contention and cancellation remain bounded, with no unconditional retries.

One application can eventually access multiple backends, but independent
connections do not automatically form one atomic transaction. Reject operations
requiring unsupported atomic scope before effects. An explicitly chosen staged
workflow must identify committed/pending/failed parts and any compensations.
Reserve a future `tablecraft-transactions` coordinator only if independently useful;
no new crate or distributed-commit guarantee is adopted now.

CRDT batches are locally transactional when the backend supports it; that does
not make peers globally serializable. Native Access shared-file collaboration
requires native locks and mixed-client qualification, separately from P2P.

Primary references: [DAO Workspace.BeginTrans](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/workspace-begintrans-method-dao),
[SQLite transactions](https://www.sqlite.org/lang_transaction.html),
[SQLite savepoints](https://www.sqlite.org/lang_savepoint.html).
