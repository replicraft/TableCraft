# Collaborative schema and application evolution

Future sync requirements; no replication/schema-epoch implementation exists.
`tablecraft-sync` owns schema compatibility negotiation, retained history and
upgrade coordination; `tablecraft-project`/`tablecraft-migrate` own their respective
version transitions. Native Access shared-file transactions remain a separate path.

Operations must identify their project, protocol and schema epoch. Before apply,
negotiate required features and schema/definition compatibility. An incompatible
peer is suspended with outstanding operations retained, not coerced into the new
schema or falsely acknowledged as synchronized. Resume only after a qualified
migration/reconciliation with explicit outcome and preserved source history.

Define authorized membership, who may propose/activate upgrades, active/offline
peer policy and recovery of stale replicas. Do not invent a quorum or automatic
upgrade protocol before proving its failure model. Retain old-epoch histories and
data until the documented reconciliation/retention policy permits removal; never
discard offline changes merely because the local application upgraded.

When using one SQLite store, commit accepted SQL projections and outbound
replication records atomically where feasible. Validate remote apply without
rerunning origin UI/Python handlers or external actions. CRDT convergence is not
relational validation, a global transaction or native ACE locking.

Script modules, definitions, dependency manifests and application releases are a
separate executable-distribution lane. Receiving them never authorizes execution
or package installation. Activate only after version/capability/trust checks;
data-sharing permissions do not authorize new code. Schema and application upgrade
plans must coordinate compatibility without treating each script edit as an
ordinary record mutation. Report partial/pending upgrades and disconnected peers
explicitly through the project health/recovery services.

Acceptance includes reordered/duplicate operations, offline old-epoch edits,
crashes between projection/history writes, malicious/untrusted script updates,
unsupported peers and deterministic post-migration convergence. See
[compatibility evidence](../testing/PROJECT_COMPATIBILITY.md).
