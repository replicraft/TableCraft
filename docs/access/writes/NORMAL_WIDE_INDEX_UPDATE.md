# Fitting Int64 and Currency index updates

Normal fixed UPDATE now maintains qualified single-column Int32, Int64 and
Currency root leaves when at least one native index uses a wider key. The
initial schema is dense and contains only these exact fixed carriers. Full
ordinary Source, constraints, alias ownership and LVAL exclusion remain required.
Nullable and descending keys, duplicate logical aliases and wider primary keys
are checked explicitly. Compound keys, ignored-NULL indexes and NODE changes
remain separate work.

Original assignments bind before staging. UNIQUE swaps are checked against the
complete final population, never an intermediate row edit. Currency stays its
scaled Currency carrier; Int64 is not a substitute. NULL transitions preserve
inactive DATA payloads. Each physical index is rewritten once; histories, roots,
inactive tails, maps, allocation and EOF remain source-relative.

The real fitting numeric kernel supplies physical patches. Independent proof
reconstructs original fixed records, signed keys plus original ROW references,
masks, free space and the complete image without staged keys or patches. The
shared fixed-record binding and proof bodies retain their existing consumers.
TCNI4, checked V1 recovery and the retained typed/SQL/CLI services are unchanged.

Forty-eight complete source views and 408 independent inert reader observations
preceded matching writes. Ninety-six native commits, original-wire comparisons,
independent proof/forgery, recovery/resource controls, 56 retained typed/SQL saves
and eight CLI saves/readbacks pass. See the
[execution record](../../testing/results/ace16-normal-wide-index-update-writes-2026-11-01.json)
and [source qualification](../../testing/ACE16_WIDE_INLINE_CLOCK_SOURCES.md).

These are portable checks. Native Microsoft Access save/reopen, wider retained
NODE partitions, splits and broader schema/index combinations remain work.
