# Ordinary ACE fixed-field writes

`NativeImage::plan_recoverable_ordinary_fixed_updates` accepts original fixed-field
groups only after fresh, complete content validation. It is a separate normal
engine route from positive BeforeChange updates and engineering previews.
`NativeJournalEditor`, the retained `OfflineWriteSession` and
`query journal-update` now consume it through their existing lifecycle.

## Current admitted family

The initial family is unencoded ACE header 5/6, with the known clear header
fields, code page 1252 and General sort declaration. The standard 64 MiB captured
image envelope and existing read/work limits remain. The earlier 64-page map
ceiling is removed. Definitions fit one page; allocation uses qualified variable
inline-map windows and the sole GLOBAL record. Indirect maps still refuse.
Direct live DATA slots, free-space
counts, catalog layouts and map ownership must be complete. Unknown fields,
deleted/forwarded slots, unexplained pages or nonzero free bytes refuse.

The entire catalog must resolve four standard containers, three qualified system
tables and ordinary user tables. User names, values, table counts and physical
locations are data, not creator identities. All fifteen catalog fields and nine
variable slots are checked. Property/body/module/external-source carriers must
be absent, catalog LVAL map pairs must be complete and empty, and the relationship
and ACE system tables must be empty. Indexes, generated/calculated/complex fields,
user Memo/OLE storage, wider headers and other objects remain outside this family.

Ordinary stored scalars, Short Text/Binary and Decimal can be preserved. Writable
targets are the existing Boolean, Int32, Int64, Currency and legacy DATE fixed
carriers; numeric NULL transitions retain inactive bytes. No record resizing,
allocation or index mutation is added. These limits describe this initial route,
not the long-term compatibility target or the broader reader.

## Authority and physical proof

`ordinary_content` accounts for the header, full catalog graph, declarations,
every live record, all referenced map cells and physical EOF. It rejects duplicate
ownership and accounts for free pages. Actual scalar reads independently validate
the admitted user records. XML-looking names, Text/Binary values and inactive
payloads remain ordinary data under their declared roles.

Only a complete validation returns a private image-borrowed certificate. It binds
the actual image instance and user-table identity. Neither an empty inventory,
caller flag, creator marker nor matching digest can construct it. Preserved source
capture consumes a distinct certificate case; no empty program selection is
manufactured. Generic record protection has one shared owner, used by both this
route and existing BeforeChange consumers.

Every requested declaration/carrier/duplicate check precedes stale-value checks.
Equal assignments are ordinary no-ops in this proved program-free context. The
existing fixed-record stage derives patches; a fresh proof recaptures BEFORE,
repeats content validation and reconstructs all expected payload/mask changes
with the independent manual codec. Every AFTER byte, final constraint result and
changed-row count must agree. Staged patches are not proof authority.

## Journal and verification

`TCNI0004`/profile 4 retains the same bounded original fixed-request body, exact
carriers and BEFORE digest. No SQL, absence certificate, creator identity or final
assignments are serialized. Record-selected recovery freshly rebuilds the full
ordinary plan and compares both complete images before effects. Unknown records
and cross-profile combinations refuse; existing TCNI0001/2/3 bytes are unchanged.

The [independent sources](../../testing/ACE16_ORDINARY_CONTENT_SOURCES.md) passed
native reads and 24 exact SDK observations before mutation calls. Three content
tests cover 24 accepted and 34 refused views, image/table binding and resource
controls. Four planner/proof tests and five public planning/recovery tests pass,
including 22 literal whole-image commits. The full ACE unit and recovery batches
pass. First compile/test/lint diagnostics remain in the
[execution record](../../testing/results/ace16-normal-ordinary-fixed-2026-10-31.json).
Native Microsoft Access save/reopen, Windows, power-loss and mixed-client
qualification remain separate and unrun.

## Dynamic inline-map coverage

Inline maps borrow their original variable-length records. Checked base-relative
members determine ownership and candidate subsets even when their windows differ.
Empty windows establish no membership. The ledger allocates roles for actual
captured EOF under the caller budget; it does not derive file size from a bitmap.
Every actual page needs a represented GLOBAL bit and a complete allocated role or
zero-filled free-page check. Future GLOBAL-free bits beyond EOF are valid capacity
and never create physical roles. Owned/candidate members beyond EOF refuse.
Physical map-cell uniqueness, allocated map storage and full record/byte closure
remain mandatory.

Ten 96-page originals pass native reads and exact independent SDK observations
before mutation. Dynamic content controls include a file whose high pages all
have valid DATA roles but lack GLOBAL cells: ordinary reading succeeds, while
certification refuses. Native, retained-session and CLI consumers compare whole
images including all maps and free holes, with unchanged TCNI0004 framing and
recovery. See the [dynamic execution record](../../testing/results/ace16-ordinary-inline-writes-2026-10-31.json).

## Shared sessions and CLI

Inert metadata chooses which family to attempt; it never proves absence. The
ordinary candidate must pass complete content/source/target checking before the
adapter advertises `StoredScalar`. A failed check does not try another writer.
The private retained route keeps ordinary single Int32, empty and batch requests
on TCNI0004. Existing Jet4/positive Int32/positive fixed records remain 1/2/3.
Every subsequent actual plan and recovery repeats native qualification.

Seven adapter tests pass, including typed/SQL literal saves, equal/zero-match
requests, empty tables, stale handles, unknown-content refusals and the older
record routes. The CLI passes 22 literal complete-image saves, four equal/empty
retirements and a real create/update/TSV-readback workflow. It adds no absence
override or alternate publication lifecycle. See the
[host execution record](../../testing/results/ace16-normal-ordinary-host-2026-10-31.json).
