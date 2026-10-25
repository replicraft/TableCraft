# Explicit one-hop native linked reads

`tablecraft-access::LinkedNativeSource` is a read-only `Source` facade over
already-open `NativeSource` handles. A host explicitly authorizes each target
handle. Stored database paths, connection strings, saved queries and credentials
never open or authenticate another source. The engine's separate
[inert link inspection](NATIVE_LINKED_TABLES.md) remains available without opting
into routing.

## Composition and identity

`NativeSource::handle()` erases the concrete reader type without cloning or
opening it. Only a NativeSource can issue this opaque handle; arbitrary query
providers cannot be substituted. This permits a raw reader and an authenticated
encryption wrapper to participate in the same composition. Each distinct handle
has its own caller-issued catalog scope; repeated mappings may reuse one target
handle and scope.

`NativeLinkMapping::new` supplies six explicit values:

1. The canonical local alias and its signed root native ID.
2. The expected exact stored Database string.
3. An already-open target handle.
4. The exact stored ForeignName and expected signed target native ID.

`LinkedNativeSource::new(root.handle(), &mappings)` checks each supplied root
alias is a linked table, rereads its inert declaration and verifies scoped
identity, native ID, alias and storage against the captured inventory. Exact
Database and ForeignName text must match the host's expectations. This check does
not prove the opened handle came from that path: the explicit host mapping is
authority, and NativeSource has no filesystem identity.

The target must resolve to an ordinary local base table with the pinned native
ID. Existing source name matching applies: stored ForeignName `items` can resolve
to canonical target `Items`. The resolved canonical spelling is retained for
forwarding. No extra Unicode folding, path normalization or provider grammar is
introduced. The exposed scalar schema comes from target metadata, without
inventing a readable schema or schema agreement in the root link record.

Duplicate aliases, extra/non-link mappings, mismatching identifiers/IDs, shared
scopes across distinct handles, link chains, target saved queries and ODBC or
unknown connection providers fail setup. Access interpretation requires the
existing exact NULL/empty Connect profile. Unmapped root links remain inert:
local root queries and other mapped links remain usable, while accessing an
unmapped linked table fails explicitly. No target table name is added to the
root namespace by its mapping.

## Query and cache boundaries

Named retrieval definitions always come from the root source. Targets supply
base-table metadata and rows only. A root saved/named query can reference mapped
aliases through the same facade; target saved queries are never used as link
resolvers. Native saved-query reconstruction and query availability keep their
existing separate contracts.

Every Source capability forwards directly using the resolved target name:
scalar columns, unavailable names, scalar/collection text policy IDs, multivalue
and named-member metadata, equality hints, projected/constrained scans and
aligned collection scans. There is no fallback to a less capable default Source
method. Original filters, selected-index failures, source row order, parent/child
alignment, visitor rejection and complete-cache publication remain owned by the
target adapter and query executor. A failed or selected subset is not installed
as a complete table.

Handles and mappings remain immutable while preparing/executing. Query binding
reads target metadata without scanning parent or child records. The normal query
layer continues checking captured schemas and policy IDs. Every input must
remain stable/offline for the whole operation; distinct handles/scopes and
metadata checks do not establish a cross-file transactional snapshot. Reprepare
after changing handles, mappings or source bytes.

## Resource and policy ownership

Root and targets must borrow the exact same `RefCell<ResourceBudget>` through
`new_with_shared_budget` or its explicit-snapshot-limit variant. An owned pool or
a different borrowed pool is refused. Catalog inventories, selected link reads,
target schemas and eventual records therefore consume one cumulative native
allowance, without refills or refunds. Route allocations/setup comparisons are
charged. Each runtime alias comparison consumes a native Step and checks root
cancellation; the pool borrow ends before any target method or visitor callback.
Existing complete target caches retain their own accounting and lifetime.

Query and comparison budgets remain separate. Hosts explicitly configure target
collection/cache/index and text policy views before binding. Target field policy
identities are forwarded intact; the facade supplies no comparer, no implicit
host locale, and no new precedence for mixed policies. Root named-query/default
policy ownership is unchanged. Unknown/conflicting policies still refuse when
the existing evaluator requires comparison semantics.

A payload-free configuration span records no names, paths, connection text,
records or SQL. Typed setup errors contain no such payloads. Failed construction
returns no facade and consumed resource charges remain consumed.

## Focused verification

Ten adapter tests in `tests/linked_sources.rs` and its feature modules pass:

- Different concrete reader types bind while a target-record read trap is armed,
  then execute the expected values and preserve root namespace isolation.
- Unrelated unmapped links leave root metadata available and fail only on access.
- Exact identifiers/IDs, duplicate/extra mappings, pool/scope collisions,
  provider/ODBC declarations and chains are checked explicitly.
- Stored lowercase ForeignName resolves through the existing source name policy.
- Late target read failures publish no complete cache; successful retries and
  visitor rejection keep their existing behavior.
- Cancellation and exhausted cumulative lookup/setup work return typed failures.
- Real projected reads avoid malformed unrequested Date values; target equality
  hints and multivalue/history metadata and aligned query results are retained.

These controls establish the explicit adapter composition contract. They do not
qualify native Access linked SQL, authentication, concurrent multi-file reads,
linked writes, arbitrary providers or automatic target discovery.


## CLI target selection

`query run` accepts repeatable `--link ALIAS PATH` pairs alongside the root
`--database`. The alias names an existing native Access link; the explicit path
selects its already chosen stable/offline target. The stored ForeignName selects
an ordinary target table, using existing native name matching. Stored Database is
reread and checked as metadata, and is never used to choose a path.

```sh
tablecraft query run \
  'SELECT p.ID, p.Name FROM [People link] AS p ORDER BY p.ID' \
  --database frontend.accdb --link 'People link' backend.accdb
```

Supply one pair for each link the query needs. Unrelated unmapped links remain
inert. Multiple pairs can select the same file; this CLI currently opens each
explicit target independently, with distinct invocation-local scopes and caches,
under one cumulative native pool. The library also supports reusing one supplied
target handle across multiple aliases. Neither establishes cross-file coherence.

The root and every selected target share `--native-bytes`, `--native-rows` and
`--native-steps`; allowances do not restart per file. Per-source snapshot and
payload ceilings, the separate query budget and buffered output limit still
apply. Existing `--auto-index`, collection/decoded-attachment and cached-calculation
options configure target views too. `--index ALIAS INDEX` applies to the mapped
target table; unknown or unsupported selected indexes fail without sequential
fallback. Original query filtering and target order remain intact.

With `--collation database`, every source retains its actual stored field policy
IDs and uses the shared comparison quota. Execution keeps the root default and
the existing General legacy/2010 resolver; conflicting or unknown required
policies fail explicitly. Link routing introduces no new native precedence rule.

Global `-p` applies to explicit root and target opens. When omitted, existing
hidden terminal prompting handles each encrypted input that requires a password;
noninteractive missing/wrong passwords fail without publishing query rows. A
single explicit password does not supply distinct credentials for different
files. Provider qualification precedes target open/prompting. ACCDT root sources,
ODBC/unknown providers, target queries and link chains remain outside this path.
Every input is read-only and query output is published only after successful
execution and formatting.
