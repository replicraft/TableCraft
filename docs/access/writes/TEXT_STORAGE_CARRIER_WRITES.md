# Indexed Text writes over compressed storage

The fitting indexed Text UPDATE profile now binds qualified original compressed
and marked storage cells independently of their decoded string. Its metadata,
source admission, native key encoder, record serializer and transaction format
are unchanged.

The independent proof streams exact UTF16 units from plain payloads or the
native marker/narrow/wide encoding. It checks complete exhaustion, truncation,
mode switches and the original observed value without allocating or normalizing
it. Every consumed byte is charged and comparisons remain cancellable. This
shared native proof owner retains the existing plain-cell costs and supports
the separate INSERT/DELETE proofs without forwarding wrappers.

The source family covers compressed ASCII, Latin-1, wide-mode and leading FEFF
values under both General index versions. Original metadata remains flags 2,
extended flags 0; a compression-preference flag does not substitute for source
qualification. The Legacy and Modern General families retain their observed
different FEFF key behavior. Exact stale expectations still distinguish strings
whose native keys compare equal.

Real updates write the independently requested value within the original record
capacity. No-op and empty plans retain original compressed bytes and inactive
residue. The proof preserves exact original histories, aliases, allocation,
unrelated objects and EOF. This does not qualify arbitrary locales, unsupported
metadata, new tree shapes or Microsoft Access save/reopen.

The [execution record](../../testing/results/ace16-text-storage-carrier-writes-2026-11-01.json)
separates native engine, direct proof, resource, retained-session and CLI checks
from native Access qualification.
