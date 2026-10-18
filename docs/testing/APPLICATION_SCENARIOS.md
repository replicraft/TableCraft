# Reproducible application and agent qualification

Acceptance plan only; no scenario runner or agent-native test suite is implemented.
`tablecraft-testkit` will own deterministic fixtures/helpers when they have working
consumers; `tablecraft-compat` owns native differential qualification. Test shared
services independently of any LLM/model or MCP transport.

A representative native scenario opens a customer form, creates/validates/saves
a record, navigates to its orders and generates an invoice. Use disposable SQLite
fixtures, seeded data, controlled clocks, fixed resource/font inputs and restricted
external effects. Verify exact queries/records, Python pre/post-commit behavior,
validation failures, navigation and report totals/layout. Source projects remain
untouched and tests never replay user event effects on peers.

| Test family | Required assertions |
| --- | --- |
| Introspection | Stable IDs/revisions, explicit unknowns, compatibility status and bounded/redacted samples |
| Shared commands | GUI/CLI/SDK/MCP invoke identical semantic validation and authorization; no raw metadata bypass |
| Changesets | No source writes during staging; accurate full scope/diffs, digest-bound approval, changed payload/permissions rejected |
| Concurrency | Stale revision, competing GUI/agent edits, cross-project handles and schema epochs fail or reconcile explicitly |
| Replay / recovery | Repeated identical calls preserve one outcome; conflicting idempotency reuse fails; interrupted/uncertain apply reconciles before retry |
| Constraints / formats | Version and required-capability gates, unavailable native writers/macros and preservation blockers cannot be bypassed by an agent |
| Budgets / cancellation | Large SQL output, slow work, expired handles, memory/storage ceilings and cooperative cancellation have explicit outcomes; committed work never becomes rolled back by assertion |
| Python | Static checks separated from execution; denied execution, exceptions, timeouts, worker crashes and external-effect failures recover |
| Content trust | Adversarial instructions in records/descriptions/resources grant no permissions, change no task policy and trigger no script/install/export |
| Visual services | Semantic tree, labels/focus, clipping/overflow/missing fonts and revision-aware visual comparisons agree with artifact diagnostics |
| Native Access | Actual save/reopen and differential evidence on recorded builds, separately from independent-reader or synthetic tests |

Python projects should support Ruff, a qualified type checker and pytest through
explicit managed development environments. Ruff/type diagnostics are checking
outputs, not execution proof. pytest collection, project plugins, import hooks and
test execution can run code and require the corresponding trust/permissions.
Do not auto-install tools or execute source merely to inspect a packed project.
Packed source edits use supported APIs; unpacked `.py` editing must be reconciled,
validated and packed against its base revision before publication.

Evidence records fixture hashes, revisions, actual tools/versions/platform,
authorization scope, commands/checks and observed results. Never promote a mock
Agent Activity screen, illustrative test steps or a model's narrative into a
passing result. Follow [project qualification](PROJECT_COMPATIBILITY.md) for
release evidence, skipped environments and performance budgets.
