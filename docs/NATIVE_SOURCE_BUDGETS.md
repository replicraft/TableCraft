# Cumulative native allowances across explicit sources

`NativeSource::new_with_shared_budget` accepts a caller-owned
`&RefCell<ResourceBudget>` instead of taking an independent owned allowance.
`new_with_shared_budget_and_snapshot_limits` additionally accepts the existing
checked snapshot limits. The original owned constructors retain their behavior.

```rust,ignore
let native_pool = RefCell::new(ResourceBudget::new(native_bytes, native_rows, native_steps));
let left = NativeSource::new_with_shared_budget(left_handle, left_scope, &native_pool, &cancelled)?;
let right = NativeSource::new_with_shared_budget(right_handle, right_scope, &native_pool, &cancelled)?;
```

These are already supplied, stable native handles. A host may route its chosen
logical names to them through the existing query `Source` contract. TableCraft
does not infer paths, open linked targets, grant filesystem authority or acquire a
cross-file snapshot/transaction through this API. Each source retains its catalog
scope, configuration, snapshot ceilings and complete caches. Hosts remain
responsible for stable/offline inputs and for not replacing/refilling an operation's
shared allowance. Sources store the borrowed pool without `Rc`/`Arc` allocation.

## Accounting and cache ownership

Both sources debit the same native pool from inert catalog inventory onward.
Schema discovery, saved-definition reads/reconstruction, index configuration,
collation descriptor inspection, scalar/collection reads, existing cache-copy
allocations and calculated-field preparation keep their existing accounting through
that selected pool. No per-source or per-batch allowance is recreated. Dropping a
source, discarding caches or failing an operation never refunds successful charges;
rejected charges retain the core budget's existing behavior.

Complete native snapshots are still published only after EOF/validation. A failed
or cancelled load cannot turn a partial prefix into a complete cache. Explicit
retries use the same already-depleted allowance. Zero-I/O reuse of a complete
scalar snapshot or schema retains its prior native accounting; it does not incur
new native per-row delivery charges. Query work and result copying remain governed
by the separate query budget. A completed cache neither replenishes the pool nor
lets another source's uncached I/O bypass an exhausted allowance.

Native and comparison allowances remain separate. The per-field collation builder
rejects the exact same `RefCell` being supplied as both native and comparison pool,
before descriptor I/O or debit. It checks cancellation on that invalid-alias branch:
`Cancelled` takes precedence there; otherwise the refusal is `InspectError::Unsupported`.
Distinct pools keep their original callback/accounting paths. The owned-comparison
budget overload cannot alias by type. Sharing this native pool does not share query
execution, text-comparison or output allowances.

## Borrow and callback boundaries

The synchronous source uses fallible mutable borrowing. A conflicting host-held
mutable or immutable pool borrow reports `InspectError::Unsupported` during native
setup/saved reads, or `QueryError::Source` during query-source operations that need
the allowance. These are adapter refusal errors, not a claim of unsupported file
format or physical database damage. Already cached zero-I/O views can remain
available without taking a native allowance borrow. Configuration flags that do no
native work retain their existing behavior.

New borrowed constructors and explicit saved-read entry points observe cancellation
before borrowing the shared pool; owned paths retain their existing cancellation
sequence. Native I/O holds its borrow only during the native operation. It releases
that borrow before delivering rows to a host visitor, so a visitor may explicitly
read another source from the same pool. If reentry conflicts with an operation
currently borrowing the allowance, the existing fallible borrow returns a typed
failure rather than panicking. This API adds neither concurrency nor retries.

## Focused evidence

Seven original `shared_native_budget` integration controls pass over stable copies
of the registered inert Access example, with distinct caller-issued scopes for
independently routed sources. An explicit host router executes a real join over
two copies. Each source fits independently; the shared allowance rejects their
combined native work. Additional controls cover cached reuse with an unchanged
balance, later uncached exhaustion, setup and read borrow collisions, cancellation
and source I/O failures retaining charges, explicit row-limit failures without
partial caches, configuration/drop accounting, nested host-visitor reads, and exact
native/comparison alias refusal with a distinct-pool positive control.

The compact existing source, named-query, saved-read, collation and collection
regressions also pass (43 controls), along with strict scoped adapter Clippy.

This is an adapter resource/correctness contract. It provides no new native Access
compatibility, implicit linked-file opening, concurrent snapshot or write guarantee.
