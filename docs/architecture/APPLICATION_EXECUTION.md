# Application command and event execution

Required runtime contract; no application command/event/Python runtime is
implemented. Existing bounded query APIs and CLI paths are not retroactively
claimed to implement this application lifecycle.

Desktop, CLI, browser, Python, plugins and MCP must invoke one canonical command
service for operations such as SaveRecord. Hosts collect intent and display
results; they must not duplicate authorization, validation or transaction rules.

## Command lifecycle

1. Validate session/version/capabilities, authorization and command input. Resolve
   the exact backend/transaction scope and reject unavailable guarantees before
   performing effects. Assign an operation identity for diagnosis/reconciliation.
2. Acquire a bounded transaction through the appropriate adapter. Run required
   database rules/data macros and authorized pre-commit validation/actions against
   that transaction's state. Order must be explicit per supported event profile;
   revalidate final candidates after handlers. A handler cannot commit behind the
   command service or hold a raw engine/page pointer.
3. Cancellation, handler exceptions or validation failures before commit abort
   the operation and request rollback. Report rollback failure explicitly. Scripts
   receive scoped capabilities with bounded calls, deadlines and reentrancy rules;
   worker death cannot leave an unowned active transaction.
4. Commit through the backend. Return committed, aborted/conflict or uncertain
   status. On uncertainty reconcile durable state before allowing resubmission;
   never report success or blindly replay an unknown operation.
5. After confirmed commit, refresh UI/query state and release queued post-commit
   actions. Email, HTTP, filesystem/process and other external effects cannot
   roll back with database changes. Use durable intents/outbox where supported,
   idempotency keys and explicit retry/recovery policy. A failed external action
   does not rewrite a confirmed data commit as rollback or imply exactly-once delivery.

Post-commit handlers cannot cancel committed data. Pre-commit handlers must not
perform irreversible external effects. Interactive prompts/long tasks must not
hold native locks indefinitely; split work into explicit commands where necessary.
Mixed-backend operations follow [transaction limits](TRANSACTIONS.md), with
reported partial outcomes instead of fictional cross-engine atomicity.

Remote CRDT apply validates data invariants but must not rerun the originating
Python/UI handler or duplicate external effects. Code/definition updates use a
separate [trusted application upgrade path](../features/COLLABORATIVE_SCHEMA_EVOLUTION.md).
SQL UDF and transactional macro hooks are narrow supplied contracts; query/ACE
must not import the application runtime or Python provider.

Verification must inject failure before/during/after commit, terminate Python
workers, cancel operations, replay duplicate operations and test host parity.
Errors preserve typed causes and operation/object context at authorized boundaries;
library tracing must continue to omit user values, SQL, names and paths.
