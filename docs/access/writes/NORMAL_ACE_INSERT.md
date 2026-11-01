# Normal fitted ordinary INSERT

Normal INSERT has a separate original-request intent, native planner, retained
session and CLI consumer. It uses the shared record serializer and an independent
manual proof; engineering candidates retain their separate admission policies.

```sh
tablecraft query journal-insert "INSERT INTO Sentinel (ID, Marker, Bytes) VALUES (24, 'new', NULL)" \
  --database offline.accdb --journal fresh-insert-journal
```

The first normal INSERT profile accepts complete explicitly supplied rows on a
qualified native 17-field ordinary table with no required programs. The target
must have no relationship involving it. Unindexed targets retain their existing
route; qualified ascending, present-Int32 root-leaf indexes now use the indexed
route described below. Every operation freshly derives NoRequired and the
complete native Source; creator provenance, an empty inventory or a caller flag
cannot grant admission. The initial global 15-field catalog family remains outside this
normal INSERT profile. Passive qualified properties and existing readonly LVAL
storage remain protected. Generated/calculated/complex fields, omissions/default
evaluation, positive programs and new non-NULL Memo/OLE payloads remain outside
this first profile.

Each row is checked for exact width and declared carrier constraints before
staging. Cumulative resource limits remain active through planning and proof. Every supplied row must cover the
complete actual schema in native order; Boolean has a real value bit and does
not acquire a NULL carrier. Requested empty Text requires an explicit qualified
AllowZeroLength declaration; false remains a final field-constraint rejection.
Leading U+FEFF Text now uses the independently qualified marked-wide escape.
The three-byte prefix participates in record length, offsets and capacity proof.
A zero low UTF16 byte at a marked-wide boundary remains explicitly unsupported;
this is a current representation limitation, not a claim about Access values.

The planner serializes and reserves the entire batch against original eligible
owned DATA capacity before staging. Eligible pages are chosen in ascending
original owned-page order using actual candidate hints and protected physical ownership. A page must have both byte
capacity and an unused slot within the current 255-slot profile. New rows receive
regular visible slots in request order; existing rows, forwarding
capsules, LVALs, allocation maps and EOF remain unchanged. Capacity exhaustion
refuses without an allocation fallback. Empty batches still qualify the target.
The shared manual proof remains separate from the serializer; extracted preview
wrappers retain their original primary/generated policies. Independent proof
reconstructs complete row bytes and placement from original requests, validates a
fresh final Source and compares the whole image and count.

`NativeImage::plan_recoverable_ordinary_row_inserts` retains moved `RowInsert`
values in `NativeInsertIntent` and returns `RecoverableNativeInsertPlan`.
TCNA0001/profile1 is a distinct closed INSERT family. It stores the original table,
ordered complete typed rows and full BEFORE length/digest, never SQL, generated
defaults, final patches or an absence certificate. The first carrier set is
NULL/Boolean/Int32/Int64/Currency/DATE/Text/Binary; exact DATE bits and scaled
Currency survive without normalization. Broader engineering serializer support
does not expand this normal intent contract.

Fixed-length preparation uses the real V1 journal family.
`qualify_native_insert_recovery` consumes the same inspected `CheckedV1`, verifies
the original image binding, rebuilds normal insertion from BEFORE and compares
both complete journal images before effects. Recovery and write-free completion
confirmation then use that retained checked session. UPDATE TCNI1–7 and DELETE
TCND1 retain their separate grammars and dispatch. TCNA decoding validates the
entire closed record before heap copies, under a 64 MiB total bound, 16,384-row bound,
uniform nonzero row width ≤ 256 and 8,192-byte variable-carrier bound.

The Access consumer uses metadata-only `NativeJournalInserter` and the
existing `InsertableTable` contract. A retained `OfflineWriteSession` supplies
`insert_table` for complete typed values and `plan_insert` for the closed complete
VALUES SQL service. `StagedInsert::inserted_rows` is available before journal
setup; apply reuses the held target lease and existing outcome/retirement/cleanup
lifecycle. INSERT reports inserted rows, not matched old rows. Omission/default
and incrementing engineering services are not implicitly called.

Source qualification precedes producer execution: two unchanged genuine originals
and two independently authored INSERT images passed four native read views and
four exact SDK reports with unchanged inputs, committed as `1faae85c`. The literal
new Sentinel row `(24, "new", NULL)` occupies 21 bytes in existing DATA capacity;
the old row/OLE, nonempty property bag, unrelated objects/maps and EOF are exact.
The separate two-row source adds `(25, "tail", NULL)` in both request orders;
eight native views and eight SDK reports pass before matching batch producers.

Seven physical controls, four intent-codec tests and eight native integration
functions pass, including six literal-image commits, both request orders,
partial-write recovery and protected-image forgery rejection. Four typed/SQL
session INSERTs plus four same-session follow-on saves pass. Eight active/retired
restart contexts, two interrupted commits, metadata-only constructor checks and
pre-bundle refusals also pass. Two CLI INSERTs plus two follow-on saves and four
exact row readbacks pass. The initial regression batch passes 1,118 ACE units, 79 adapter units, 102 CLI units and
83 recovery functions. Strict all-feature, journal-only, preview-only and minimal
builds pass. Existing DELETE and EOF-update host workflows remain intact. See the
[native record](../../testing/results/ace16-normal-insert-writes-2026-11-01.json)
and [retained host record](../../testing/results/ace16-retained-normal-insert-2026-11-01.json)
for exact scope and preserved first failures.

Next compatibility work includes broader index trees/keys, AutoNumber/default behavior,
new LVAL values, allocation and positive-program INSERT. Native Access save/reopen
and Windows behavior remain unrun; this bounded portable route does not establish
complete ACE16 compatibility.


`query recover-update` and `query finalize-update` also handle TCNA bundles through
the closed action dispatcher. Neither command retries a failed INSERT decoder
as another action or replays SQL. Receipts distinguish inserted rows from old
matched rows: INSERT has no matched count and reports its actual inserted count.
See [offline sessions](OFFLINE_WRITE_SESSIONS.md) for lease, failure and namespace
requirements.


## Leading U+FEFF follow-on

Six native/SDK source views distinguish the escaped requested value from a
readable alias that loses the leading character. The independent proof now
reconstructs the escape without importing producer encoding helpers. Two native
commits and two CLI saves/readbacks match the literal whole images; all six
pending/partial/committed alias journals refuse before effects. Ten normal INSERT
controls, 329 existing INSERT tests and strict all-feature/journal checks pass.
The CLI readback verifies four UTF16 units and signed first-character code -257.
See the [separate execution record](../../testing/results/ace16-normal-insert-feff-writes-2026-11-01.json)
for focused scope and preserved first preview assertion failures.

## Indexed existing-page insertion

The indexed route requires complete ordinary scalar declarations and checked
single-column Int32 indexes with present values, ascending direction and one
qualified root leaf per physical index. All logical aliases must agree. Catalog
ownership and live LVAL checks exclude conflicting page roles. Ignored-NULL,
composite, descending, nullable-key, foreign-reference and interior-node profiles
remain outside this initial insertion route. Empty batches retain these checks.

The shared index builder reserves all rows and keys before copying candidate
bytes, maintaining original topology without a split or allocation continuation.
Each physical index is updated once even when several logical aliases expose it.
Its historical counter increases by distinct new key families, rather than by
inserted rows or alias count. Uniqueness and counter overflow refuse before staging.
Existing slots, inactive leaf tails, maps and EOF remain protected.

Independent proof replays DATA placement from original capacity and complete
requests, retaining each resulting ROW identity at that request's placement.
It manually encodes and orders key/ROW entries, rebuilds leaf framing and
historical deltas, recaptures final Source and compares the complete image.
It accepts no producer reservations or prepared keys. Physical row enumeration
order cannot substitute for request-to-ROW association.

Two genuine originals and four independently authored AFTERs passed six native
source views and 36 independent SDK reports before matching producer calls.
The single request adds one distinct key to each physical index; the two-row
batch adds two primary keys but one secondary family. Four native commits,
typed/SQL same-session saves and CLI saves match those complete images. Recovery
checks include authored mixed DATA/INDEX/definition targets, pending rollback,
committed mixed-target refusal and protected-byte forgeries. Duplicate-key
refusals retain exact typed errors and CLI exit status. See the
[source contract](../../testing/ACE16_NORMAL_INDEXED_INSERT_SOURCES.md) and
[indexed write record](../../testing/results/ace16-normal-indexed-writes-2026-11-01.json).
These checks do not establish native Microsoft Access save/reopen compatibility.
