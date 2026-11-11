# Retained index removal

`NativeImage::prepare_retained_index_drop` prepares a separate
`NativeIndexDropCandidate` for qualified ACE16 retained tables. It removes one
ordinary nonunique single-LONG index declaration. The candidate supplies no
journal, Runtime, CLI or GUI save authority.

## Source and preservation

Complete retained schema, program/dependency, ownership and index-membership
checks precede construction. The selected LONG field must have its ordinary
fixed/updatable declaration. Primary, unique, foreign-reference, compound and
other key profiles remain outside this operation. The selected index-name
namespace is ASCII-qualified; this is an implementation boundary rather than an
Access naming restriction.

When another logical alias still references the physical tree, removal changes
only the selected logical descriptor and name. Every surviving logical identity,
physical tree and map stays intact. Removing the final alias also removes the
physical descriptor and historical counter, compacting surviving physical
references without renumbering their logical identities.

The final-alias route certifies the entire retired footprint, including the root,
all descendants and dedicated usage-map storage. It requires exclusive ownership
and a qualified single-row map host. Shared map containers are not reclaimed
without their own liveness proof. The existing row-update retirement service,
which forbids root retirement, keeps its separate contract.

The new producer zeroes the certified retired pages and marks exactly those pages
free. This is an explicit construction policy, not a claim that Access produces
identical retired bytes. File length is preserved. DATA, LVAL, catalog chronology,
generated counters and unrelated index/map bytes remain unchanged.

## Independent checking

Fresh AFTER inventory and retained-source checks revalidate the resulting file.
The proof compares surviving parsed index identities and references, checks the
monotone definition-byte splice and independently checks complete zero/free and
protected-byte relations. It does not accept the staged buffer as its own oracle.
Cancellation and cumulative resource checks apply throughout source admission,
page ordering, retirement and proof.

The [native checkpoint](../../../tests/results/indexes/retained-index-drop-candidates-2026-11-11.json)
records six DROP cases alongside existing CREATE regressions and independent raw
controls. Coverage includes empty, NULL/duplicate, multi-leaf and 200,000-row
three-level populations in both directions, a generated-primary/Currency/Unicode
companion, surviving aliases and physical descriptor compaction. It does not
qualify native Windows/Access behavior, durable recovery or application commands.

Later index creation has a separate identity-allocation contract. Preserving a
surviving logical ID while reducing descriptor count can leave that count already
occupied; an existing count-based CREATE request must not silently change its
replay language or renumber the survivor to hide the collision.
