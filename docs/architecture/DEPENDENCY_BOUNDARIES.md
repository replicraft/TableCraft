# Enforced dependency boundaries

[The checker](../../scripts/check_dependency_boundaries.py) uses locked/offline
Cargo metadata and an explicit allowlist for the twelve existing packages. It
checks every declared workspace edge, including optional, renamed, target-specific,
build and development dependencies. Libraries cannot import application packages;
unreviewed packages/local dependencies fail rather than silently inheriting access.
Indirect workspace paths necessarily contain at least one checked forbidden edge.

Current rules keep core/schema/model independent of hosts; I/O imports only core;
DB contracts import core/schema; SQL has no workspace dependency; expression
semantics may import SQL/core; query uses shared contracts but no concrete adapter
or engine. ACE may use reusable query/semantic contracts and I/O/foundation,
but never DB, adapters or hosts. Access/SQLite adapt engines into shared/query
contracts, and CLI composes these libraries. The allowlist is slightly broader
than today's graph for already accepted directions; it is not a feature claim.

Access also consumes `tablecraft-io` for bounded deployment-package reads. CAB
parsing/decompression stays in Access; the generic I/O crate gains no container
or native-format knowledge. ACE and query have no CAB dependency.

```sh
python3 scripts/check_dependency_boundaries.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Python 3.11+ is an engineering-only prerequisite; no application Python runtime
is installed by this check. CI runs these commands before Rust build checks.
The guard checks local package ownership, not third-party transitive/feature
closures, source-level API exposure, unsafe code or runtime correctness. CLI
GUI/network/runtime isolation and upstream AST privacy still require feature/tree
review and code review; passing this check is not proof of every architectural rule.

Future packages need reviewed rules with their first working implementation:
project serializers sit above shared definitions and SQLite services; Python sits
behind application services, never in the reusable engine/query dependency closure.
Pure automation/macro contracts may serve engines but cannot import concrete
runtime/UI/adapters. SQL UDFs use supplied lower-level contracts. Domain directory
placement alone does not make a reusable contract an application host dependency.

Keep semantic editable definitions, preserved opaque native payloads and derived
runtime/render state distinct. The future canonical application model must not
become a central crate importing all application domains or upstream SQL ASTs.

The prospective edit boundary depends on shared models/lower contracts, not
runtime, GUI, MCP or application hosts. Runtime orchestrates apply and protocol/
UI adapters consume the same services. `tablecraft-edit` has no manifest and
requires a policy review at introduction like any new package. The existing
checker was not relaxed to permit speculative agent/runtime dependencies.

## Provider ownership

Shared DB contracts reside in `foundation/`; Access and SQLite integrations
reside in `backends/`. Future `adapters/` packages connect external systems rather
than owning native storage. This relocation changes no allowed dependency edges.
Shared task/link services compose providers through narrow contracts; provider
packages must not import runtime or UI. The standalone ACE engine remains
independent of DB and all concrete providers. See [External Data](../features/EXTERNAL_DATA.md).
