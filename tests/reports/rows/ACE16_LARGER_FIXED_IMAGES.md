# Larger fixed-update images

The independently authored larger-image SOURCE corpus passes inert qualification:
12 original/complete expected pairs (24 images) and two additional over-cap
originals. It covers both ACE 16 header profiles, 64 MiB, the first page beyond
64 MiB, 80 MiB, and a preserved Boolean source four pages beyond 64 MiB. This is
inert SOURCE evidence. Default `NativeImage::capture` retains its 64 MiB ceiling.
Explicit `capture_for_fixed_updates` now accepts validated positive limits up to
80 MiB and retains that policy privately. The current phase-3 continuation permits
only the existing qualified fixed single and nonallocating batch engineering
services. Other mutation/preparation routes and normal plans remain denied under
that policy, even on a small file or an empty/no-op request.

Indexed fixed sources contain three rows and two aliased physical integral
indexes. High-address variants relocate DATA, INDEX roots and their exact ROW
references; the 80 MiB high variant also places its INDEX-map container and GLOBAL
bitmap above 64 MiB. Every retained page, including opaque tail pages, has explicit
allocated coverage through a complete indirect GLOBAL map. Preserved Boolean
sources reuse the existing independent sixteen-field model with calculated,
complex and LVAL siblings. The expected change remains one Boolean bit; its
unrelated values and stored bytes stay intact.

The qualifier checks header/catalog/schema, original rows, scalar index and
constraint coverage, exact page/map/ROW identities, preserved Boolean projections,
and complete byte differences. Moved old DATA and INDEX bodies remain opaque
originals. Every abandoned page and every byte outside the selected differences
is checked for equality. Cases are constructed sequentially to bound retained
memory. The default capture API continues to refuse every above-64-MiB case.
Explicit capture uses the same complete-byte retention, stable/offline source
contract, checked EOF/header/catalog and cumulative resource/cancellation controls.
The API is available in the standalone engine without the preview feature; caller
limits do not change allocation, encryption, journal or normal-save ceilings.

The first compile failed because the test treated `IndexInspection` as its index
collection. The correction used its actual accessor. The first runtime then
found that the initial expected constructor retained a change in abandoned low
DATA. A separate named expected derivative restores abandoned DATA and INDEX
bodies from the immutable original source. The original constructor and original
source bytes remain unchanged and callable; the derivative qualified before any
new capture policy or writer existed. These failures and the passing rerun are
preserved in the [execution record](results/ace16-larger-fixed-image-sources-2026-10-30.json).

At the capture-only phase-2 checkpoint, extended capture retained all 24 images
byte-exactly. Existing source
owners certify 20; four header-5 rich sources explicitly refuse with
`Unsupported(Column)`. Their native fields 7/8 are 42-byte Extended Date/Time,
whose scalar contract requires the ACE 2019 profile. The first capture-source
check incorrectly expected those to qualify. Only its expectation changed;
original source and expected bytes stayed frozen. Boolean-only inert projection
in the earlier SOURCE gate did not establish full preserved-source admission.

At that phase-2 checkpoint, all 30 public mutation/preparation entries refused
the extended policy on small Jet 4 and both ACE 16 header profiles. Limits tests pass for exact/one-below
Bytes, Rows and Steps, exact source-size ceilings, and the final observed cancel
poll. Normal and standalone minimal checks pass. The capture-only regression
round passes 541 native mutation, 47 preservation and 37 fixture-consumer
functions, with two export helpers ignored. All-target/all-feature native strict
checks, native formatting, documentation links and dependency boundaries pass. See its
[capture execution record](results/ace16-larger-fixed-image-capture-2026-10-30.json).
The earlier SOURCE strict/format/link/boundary results remain recorded separately.

## Phase 3: fixed single and nonallocating batch updates

`preview_ace16_fixed_updates` and nonallocating `preview_ace16_batch_updates` now
accept the retained fixed-update-purpose policy under their existing ACE 16
format, subversion, field, index and constraint gates. The validated ceiling
remains attached to the original image, including for small sources. Default
capture remains 64 MiB; caller work budgets and other mutation/allocation ceilings
do not increase. No new public preview setter or mutable image conversion is
introduced. These ACE preview methods refuse Jet 4 with Format; normal Standard
Jet 4 plans keep their separate contract.

For ordinary fixed tables, the ExtendedFixed-only source gate derives the affected
column union from original names, checks complete body/map exclusivity and
captures every original logical root and its exclusive final record, including
named groups with no assignments.
GLOBAL must represent the header, map storage, selected definition, all owned
DATA and physical INDEX map/body pages as allocated. Physical aliases are
certified once. Read protection grants no map-record rewrite authority. Rich
Boolean updates retain their existing stronger reconstruction/preservation proof.
The shared [fixed DATA/INDEX ownership guard](ACE16_FIXED_INDEX_BODY_OWNERSHIP.md)
also protects Standard fixed routes; this separate P0 correction was required
before accepting the combined feature.

Independent complete-image tests exercise ten eligible original/expected pairs
(eight scalar and two header-6 rich) through both fixed services: twenty exact
changed candidates and twenty exact no-ops per run. Two header-5 rich originals
remain Column refusals through both services. The original source/expected
constructors, including the separately qualified abandoned-body correction,
remain unchanged. The source-only small resource pair and reader-valid role-alias
negative qualified before the producer. Late stale expectations, repeated rows,
final duplicate keys and source aliases fail without a candidate. A completely
empty group list has no target and returns an exact no-op; a named empty group
still certifies its source.

Preserved candidates are reconstructed from original requests before a private
same-length recapture copies the exact original policy and ceiling. Standard
stays Standard; extended stays extended. Length changes refuse; candidate bytes
or size never infer authority. Both fixed paths pass exact/one-below Bytes, Rows
and Steps controls and final cancellation, including original capture in the
allowance. The other 28 previously existing mutation/preparation roots stay denied,
including allocating splits, variable/LVAL operations, cascades, guard preparation,
INSERT/DELETE and normal plans.

The six-function native suite and remaining-route denial test passed before the
shared P0 guard. A separate Standard diagnostic then exposed reader-valid body
overlap acceptance; its failed result is retained in the P0 history. After that
shared correction, the seven-function larger native suite passed again, including
the diagnostic and all twenty candidates/twenty no-ops. See the separate
[phase-3 execution record](results/ace16-larger-fixed-updates-2026-10-30.json).
The earlier SOURCE and capture-only JSON records remain historical evidence.

The explicit [SQL host](../access/writes/SQL_ENGINEERING_COPIES.md) selects only
`Fixed(Fitted)` with `--image-bytes`; it does not widen default routing. All these
outputs remain nonjournalable engineering candidates. Allocation growth, native
Access save/reopen comparison, required-macro qualification and normal-save
admission remain open.
