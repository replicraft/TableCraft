# Ordinary indexed GUID updates

The ordinary fixed UPDATE service admits a manual, nullable single-GUID index
with retained single-Int32 owners in the qualified root-leaf layout. It checks
complete native Source, program-absence proof, all physical owners and logical aliases,
original values and every target before producing a candidate. Read admission
alone does not grant indexed mutation authority. Indexed Int32 targets in the same
operation remain refused by this GUID route.

Canonical GUID values remain original request data in the existing TCNI0010
intent. Native DATA uses the mixed-endian 16-byte layout; GUID index components
use canonical byte order with their directional tags and a physical ROW tie
breaker. NULL is distinct from its retained inactive payload: clearing presence
preserves all 16 bytes. Exact duplicate values are allowed by the qualified
nonunique secondary, with complete key-plus-ROW ordering.

The fitting rewrite replays each physical population once, preserving aliases,
the untouched primary index and all bytes outside the selected DATA spans and
qualified active secondary framing. It may increase a common prefix to fit;
source-relative inactive tails remain unchanged. Table count and both historical
index counters stay 158 in the qualified inputs. This route performs no page
allocation, index growth, retirement or counter generation.

Unindexed Score and indexed GUID replacements can compose atomically in either
written column order, including the same original row. The complete groups reach
DATA binding/staging and independent reconstruction; only indexed GUID effects
are selected for key rewrites. A row changed by both groups counts once.

Fresh proof independently reconstructs DATA, complete GUID/I32 populations, native
framing and the final complete Source. The original TCNI10 request goes through
the same checked V1 journal; cold recovery requalifies the original request and
images. A failed action/dialect is never retried as another. Old TCNI4/8/9 remain
closed even for a named-empty GUID target.

## Evidence and current qualification

Four unchanged SDK-authored originals cover both header families and GUID sort
directions. Request-first laws bind 28 ordered requests to 24 unique independent
AFTERs: present replacement, duplicate replacement, swap in both orders, NULL
clear and filling the final NULL with a new or duplicate value. The swap orders
share their final image. Native read/Source qualification passed 28 views; the
bounded independent SDK reader passed 28 observations with 58 saved artifacts.

The compiled producer's two primitive key-wire functions and reader/journal/preview
profiles passed. The shared testkit fixture extraction passed its three existing
original/read/Score functions and was committed separately as b3116d80.

Matching acceptance passed: 28 exact native commits and original
wires; 28 direct positive plans, 28 count refusals and 468 protected-byte forgeries;
18 complete public resource/cancellation contexts; 48 representative cold recovery
states, including 12 committed-target mismatches; 28 ordered typed saves and 24
distinct CLI saves. GUID+Score composition adds 8 native/direct/typed/CLI contexts
at each applicable boundary. The two representative mixed resource contexts are
included in 18, not added again. Twelve distinct noops, 8 stale witnesses, 8 carrier
refusals, 16 indexed-I32 target guards, 12 old-dialect empty guards and 28 malformed
original/preflight controls remain explicit. Native readbacks consume actual saved
target bytes; independent frozen AFTER files remain the exact image oracle.

All 25 retained runtime functions passed, followed by strict all-target checks
for ACE, Access, CLI and testkit, formatting and dependency guards with 17 tests.
See the [execution record](../../testing/results/ace16-normal-indexed-guid-update-2026-11-03.json).

Known Required/UNIQUE/primary GUID declarations are checked structurally, but
positive mutation corpus qualification for those variants remains work. Tuples,
ignored-NULL GUID keys, NODE growth and broader key compositions also remain work. These originals contain no live LVAL payload role; their malformed
page-role controls do not constitute new LVAL-overlap qualification. Native
Microsoft Access save/reopen, Windows locking, mixed-client and power-loss
comparison have not been run.
