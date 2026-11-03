# GUID UPDATE sources and admission

This audit separates original reads, complete source admission and mutation.
It reuses two unindexed wide-scalar originals and four single-GUID ASC/DESC
originals from the [earlier GUID tuple evidence](results/ace16-insert-guid-tuple-index-native-2026-10-28.json).
No reference reader, database application constructor or write producer runs.

All six originals pass complete scalar/raw-record checks. The indexed sources
also pass manual GUID key/ROW, bitmap, owner, alias and historical-counter checks,
plus their actual GUID All/Null/Equal reads and complete primary reads. Their
logical NULL row retains the previous GUID's sixteen inactive bytes; it remains
separate from that present GUID during selection. The full GUID root is at its
observed capacity. Read success does not establish growth or mutation support.

Both unindexed originals pass actual update-program absence and canonical ordinary
Source capture. Four additional in-memory states independently replace the native
sixteen-byte payload or clear only its presence bit while retaining that payload.
All four pass complete native reads and both admission services. Restoring presence
returns the original exactly; it creates no additional state or repeated native
observation. These six unindexed states have no new external-reader/native Access
interpretation claim.

All four indexed originals refuse both admission services with
`Unsupported(ActiveContent)`. A separate actual-helper probe passes standard
capture, schema, catalog, present properties and catalog LVAL declarations, then
first refuses dense integral definition validation. Static tracing identifies
the selected index declaration's missing GUID-width arm. This is an inability to
qualify the source, not evidence that active code exists. Later stages and full
program absence remain unestablished for these indexed originals.

Three ignored functions and ACE all-target/all-feature strict checks pass. The
[execution record](results/ace16-guid-update-admission-2026-11-03.json) preserves the
first test-only compile failure: an extra selection argument to the complete-read
API. Correcting that call changed no production code, source or byte law.
No GUID UPDATE save, intent profile, journal recovery or native Access save/reopen
is qualified by this audit. A later source continuation must repeat full admission
before any matching indexed write claims.


## Later selected GUID Source continuation

The [retained GUID Source service](../access/writes/NORMAL_RETAINED_GUID_SOURCE.md)
now passes complete admission for all six originals and four exact unrelated
Score plans. The baseline refusal/stage observations above remain historical;
the old integral declaration family still stays closed to GUID keys.

## Independently authored indexed UPDATE outcomes

A later request-first model constructs 24 unique indexed AFTER images for 28
requests over the four retained single-GUID originals. Both sort directions and
header families cover replacement, duplicate values, both orders of a two-row
swap, NULL clear and filling the last NULL with a new or duplicate GUID.

The model manually reconstructs native DATA bytes and secondary GUID/ROW keys,
entry boundaries, common prefixes and free space while preserving inactive tails.
The I32 primary, complete unselected bytes, row identities, maps, catalog, EOF and
historical counters remain unchanged. All 158 original records and both physical
index populations are checked before construction. Forward/reverse swap requests
share one final image per original; they are not counted as additional states.

A separate native gate passed all four original and 24 AFTER views, with complete
raw/row checks, all four aliases, GUID All/NULL/equality and primary reads, unchanged
historical counters, full program-absence proof and canonical ordinary Source.
One focused ignored function, ACE all-target/all-feature strict lint and formatting
passed. See the [execution record](results/ace16-indexed-guid-update-sources-2026-11-03.json).
These are source/read premises for later mutation qualification. This gate invokes
no writer, save or journal; Microsoft Access save/reopen remains unrun.
