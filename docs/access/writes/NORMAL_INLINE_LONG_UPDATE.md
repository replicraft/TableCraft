# Fitting inline Memo and OLE updates

Normal fitting UPDATE now edits qualified ordinary inline Memo/OLE fields through
the existing typed, SQL and CLI services. Memo uses Text, OLE uses Binary, and
supplied NULL stays distinct from an inline empty value. Optional fixed sibling
groups share the same original record. TCNI5 and checked V1 recovery are unchanged.

Every selected long field must have complete checked NULL/inline storage and
zero external owned, candidate, shared or chunk inventory, including for empty
and no-op requests. Descriptors have zero reserved words and at most 64 payload
bytes. External targets refuse even when clearing them: dropping a descriptor
cannot orphan a live chain. Readonly external siblings retain their existing
preservation rules. Empty Memo authoring needs an actual AllowZeroLength
declaration and the normal field validator still enforces its value.

The existing native long encoder supplies inline payloads. Fitting counts the
12-byte descriptors and actual payloads together under the record's original
shared capacity. Independent insertion payload proof moved byte-exact to its
neutral serialization owner; all consumers import that owner directly. Fitted
proof independently derives the descriptors and full record, then checks fresh
Source and LVAL inventories. Footer, slack, maps, unrelated bytes and EOF remain
protected; no-op requests preserve the original storage representation.

Auto selection chooses fitting only when all long targets fit. Needed growth
and explicit growth requests with a long target refuse before staging. External
reuse/allocation and long-value retirement remain unfinished work.

The 28 source contexts passed native and independent readers before matching
writes. Twenty-six literal native commits, independent proof/forgery,
recovery/resource checks, 42 retained typed/SQL saves with empty follow-ons and
sixteen CLI saves/readbacks pass. SQL coverage uses honestly expressible Memo
and OLE-NULL cases; there is no invented Binary literal or implicit coercion.
See the [execution record](../../testing/results/ace16-normal-inline-long-update-writes-2026-11-01.json)
and [source qualification](../../testing/ACE16_WIDE_INLINE_CLOCK_SOURCES.md).
Native Microsoft Access save/reopen remains unrun.
