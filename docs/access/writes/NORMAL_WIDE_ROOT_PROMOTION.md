# Coordinated wider numeric root promotion

Normal complete-row INSERT now promotes two overflowing Int64 or Currency
secondary root leaves in one TCNA4 operation. The qualified schema has three
manual fixed fields: Int32 primary, Int64 or Currency, and Int32. Three physical
indexes cover the primary and ascending/descending wider keys; consistent logical
aliases retain their shared physical owner.

Source/request overflow bounds select the route before physical planning.
Complete Source, catalog ownership, required-program and LVAL exclusion checks
precede candidate construction. All supplied rows, final memberships, uniqueness
and distinct historical deltas are prepared before staging. DATA placement uses
existing owned capacity. The primary retains its original uncompressed framing
and must still fit.

Both wider roots become NODE pages with two fresh leaves each. Four contiguous
EOF INDEX pages are assigned right then left for each physical owner in ascending
owner order. Their original GLOBAL and INDEX map cells must already represent
the allocation with the required polarity. Shared-byte claims coalesce under one
original authority; arbitrary page selection and map expansion are unsupported.

Independent proof reconstructs records, request-aligned ROWs, complete sorted
memberships, histories, partitions, full key/ROW fences and all map effects from
BEFORE and the retained requests. It compares the complete image and captures
fresh final Source. TCNA4 retains its complete-row wire format and existing V2
recovery; no caller-supplied split mode or preview conversion grants permission.

Four literal commits pass direct, journal, retained typed/SQL and CLI acceptance.
Recovery checks include 56 pending partial-image shapes, distinct GLOBAL-only
and INDEX-owned-only map states, committed mixed-image refusals, cold host
restarts, forgeries and exact resource/cancellation controls. Existing Int32 root
split regressions pass. The
[source qualification](../../testing/ACE16_WIDE_ROOT_PROMOTION_SOURCES.md)
preceded matching writes.

Primary-prefix growth, DATA plus wider-index allocation, nullable/compound keys,
retained wider NODE insertion, recursive splits and map expansion remain separate
work. Portable complete-image and independent-reader checks do not establish
native Microsoft Access save/reopen compatibility.
