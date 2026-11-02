# TCNA4 complete original-row INSERT

Complete supplied-row INSERT now uses TCNA4 through the native planner, shared
offline sessions and CLI. Focused and broad local checks pass; the
[execution record](../../testing/results/ace16-complete-insert-writes-2026-11-01.json)
retains exact commands, counts, hashes and the first failed lint receipts.
The independent [wider scalar source qualification](../../testing/ACE16_WIDE_SCALAR_INSERT_SOURCES.md)
has passed 32 native source views and 32 exact SDK reports before these producers.
That read evidence does not establish native Access or Windows save/reopen.

## Original data and native admission

`NativeImage::plan_recoverable_ordinary_complete_row_inserts` retains the caller's
original table and ordered, complete `RowInsert` values with the complete BEFORE
length and digest. `NativeCompleteInsertIntent` is data, not a write capability.
Decode/new validates framing and carriers; every plan and recovery separately
captures fresh native content and proves the complete candidate.

The ordinary route uses the [action column-identity profile](NORMAL_COLUMN_IDENTITIES.md)
and requires the existing native seventeen-field catalog admission,
selected-table `NoRequiredTablePrograms`, full Source/ownership and applicable
constraint checks. No creator provenance, empty inventory, caller absence flag,
fake event program or preview candidate conversion supplies permission. Generated,
calculated, complex, linked and unresolved required-content policies remain in
their existing owners. Complete requests do not generate defaults or AutoNumbers.

TCNA4 adds exact Byte, Int16, Single, Double, GUID and Decimal carriers to the
existing NULL, Boolean, Int32, Int64, Currency, DATE, Text and Binary grammar.
Shared native record serialization and a separate manual record proof already
own their stored encodings. Native fixed widths, flags, extension flags, overlap,
variable coverage, properties and final constraints remain fresh checks. Every
schema qualifies even for empty or NULL requests. Boolean has a value bit and
requires Boolean data; it is not a nullable numeric payload.

Single and Double preserve all IEEE bits, including signed zero, subnormals and
NaN payloads. DATE remains its distinct validated legacy carrier. GUID bytes are
explicit data, without UUID generation or object-reference semantics. Decimal
retains signed coefficient and scale; core construction checks its 96-bit envelope
and scale 0–28, while the actual native field separately enforces its precision
and exact scale. No rescaling, float normalization or implicit scalar conversion
occurs. SQL assignment retains its exact-carrier rules and query nonfinite limits.

## Source-derived placement

The unindexed complete planner binds and encodes all original rows before reserving
any candidate bytes. It visits original owned capacity in native order for each
written request. When no current capacity fits, it reserves the next contiguous
EOF DATA page. The number of pages is the count required by this deterministic
request-order first-fit policy, without moving old rows or reordering requests.
It is not a claim of globally optimal bin packing.

Every new page must already have represented GLOBAL FREE, owned and candidate
cells with the required original polarity. One original catalog census proves
physical record/storage exclusivity, protects other roles and excludes live LVAL
aliases. Claims sharing an original byte coalesce only within the same map role.
No map growth, map creation, arbitrary-page allocation or unused appended page is
introduced. BEFORE and AFTER retain the existing 64 MiB image ceiling.

Existing indexed placement delegates to the current fitting index planner and
its actual INSERT/DELETE action facts. That preserves existing I32/I64/Currency
key profiles, aliases, histories, constraints and ownership. Qualified
[coordinated wider root promotion](NORMAL_WIDE_ROOT_PROMOTION.md) now has a
separate source-selected path with four represented EOF INDEX pages and existing
DATA capacity. Qualified
[fitting Short Binary secondary roots](NORMAL_BINARY_INDEX_INSERT.md) now also
use this path; it does not widen
indexed UPDATE's separate I32 target policy. Existing unindexed fixed-scalar
UPDATE support is unchanged. The indexed branch retains its existing
record-declaration policy. The new wider-declaration witnesses are unindexed;
no wider key, index allocation or split is granted by the carrier grammar.

Independent proof freshly captures BEFORE, manually encodes original requests,
replays its own request-aligned capacity decisions and reconstructs allocation
cells from original bytes. It compares the entire result and changed count, then
captures final Source. Producer arrays, patches and page reservations are not its
authority. Wider-plus-multiple-EOF composition has separate evidence requirements;
the 32-view scalar source phase contains existing placement and one EOF page.

## Wire and recovery

TCNA0004/profile4 retains the 68-byte complete-row header, uniform bounded rows
and length-delimited cells. It stores no page count, storage mode, final patches,
expanded defaults or permission. Limits remain 64 MiB per encoded intent, at most
16,384 rows, width 1–256 and the existing variable-carrier cap. The entire frame,
name, padding, UTF8 and carriers validate before decoded allocation, with paid
scans/copies and cancellation.

Legacy tags 0–7 are unchanged. Only TCNA4 adds these request representations:

| Tag | Carrier | Exact payload |
|---|---|---|
| 8 | Byte | 1 byte |
| 9 | Int16 | 2 signed little-endian bytes |
| 10 | Single | 4 little-endian raw IEEE bits |
| 11 | Double | 8 little-endian raw IEEE bits |
| 12 | GUID | 16 canonical `Guid::to_bytes()` bytes |
| 13 | Decimal | signed little-endian coefficient in 16 bytes, then scale |

Request GUID/Decimal framing is distinct from their native field wire. TCNA2's
tag 8 means omitted only inside that closed original-presence protocol.

The proved plan chooses V1 for equal lengths and V2 for positive page-aligned
growth. Fixed-only `prepare` rejects a growing plan before effects; `prepare_insert`
uses the existing fixed/resizable prepared sum. Loading reads exact journal magic
once and retains one `CheckedImage::Fixed` or `Resizable` session. It never retries
a failed inspector or decoder under another family.

Native qualification verifies the original digest and freshly replans the same
request body. V1 must agree with zero source-derived growth; V2 must agree with
positive exact derived growth. Both complete images must match. Equal-length V2
can be valid generic I/O framing and still fails native qualification. Likewise a
valid fixed journal cannot authorize an original request requiring EOF capacity.
The same checked session performs recovery or completion confirmation. Committed
mixed targets refuse without repair; pending/partial rollback and durable final
confirmation retain the existing generic contracts.

The actual complete typed and SQL host emits TCNA4. Its constructor stays
metadata-only and does not grant mutation admission. True omitted fields retain
TCNA2; explicit native TCNA1 and TCNA3 APIs, grammars and recovery remain unchanged,
including their old declaration refusal for empty/NULL wider requests. Existing
offline lease, publication, known-outcome retention, retirement and write-denied
confirmation are shared. INSERT reports inserted rows and has no matched count.

## Evidence status

The scalar source phase has passed one exact native read gate over 32 views,
one exact export test, 32 SDK reports and saved verification without retries or
changed inputs. The observer's nine/ten-page and synthetic-class preflight
corrections were retained before the first read; no expectation was retuned.

The focused integration checks have passed:

| Boundary | Passed test functions |
|---|---:|
| Generic checked journal framing | 5 |
| TCNA4 codec | 4 |
| Legacy TCNA1 codec | 4 |
| Complete physical and independent scalar/multipage proof | 6 |
| Wider scalar public native plans, recovery, resources and journal-kind refusal | 7 |
| Wider primary native acceptance | 3 |
| Primary policy and direct proof | 2 |
| Dense numeric public native acceptance | 2 |
| Wider scalar Access / CLI acceptance | 1 / 1 |
| Retained-session complete INSERT restart | 2 |
| Dense numeric Access acceptance | 2 |
| Existing indexed INSERT Access regression | 2 |
| Integral indexed Access / CLI regression | 2 / 2 |
| Omitted/default INSERT Access regression | 2 |
| Retained-NODE INSERT Access regression | 1 |
| Generic I/O checked-borrow documentation tests | 4 |

These counts are test functions, not a count of distinct images or compatibility
profiles. The wider native batch includes both journal-kind mismatch tests;
the six physical tests include the three independent scalar proof functions.
Their focused receipts are retained in `first-results.json` and
`acceptance-first-results.json` for the complete INSERT integration batch.
A hex fixture lint correction and its first failed receipt are retained; no
production failure or source expectation change was needed for these focused
passes. Strict all-feature, journal-only, preview-only and minimal checks pass.
The broad batch passes 1,142 ACE units, 104 I/O units, 79 adapter units,
102 CLI units and 90 default recovery functions. Ignored fixture-dependent
functions are qualified only by their explicitly executed focused receipts.

The wider scalar witnesses cover existing capacity and one EOF page. The separate
multi-page witnesses use their source-qualified I32/NULL rows. Their passing
results do not establish every combination of wider carriers and multi-page
placement. Native Access execution/save/reopen and Windows locking/durability
remain separately unrun.
