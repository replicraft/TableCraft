# Normal Short Binary index insertion

Complete ordinary-row INSERT now maintains qualified nullable ascending and
descending Short Binary secondary root leaves through the native planner,
retained offline sessions and `query journal-insert`. The actual complete host
uses the existing TCNA4 request and fixed V1 journal. Native Access save/reopen
and Windows durability remain unrun.

## Admission and physical scope

Fresh native17 catalog, required-program absence, constraints, all-local ownership
and preserved Source checks remain mandatory. Selected native declarations must
admit the actual Short Binary width and exact flags. Complete checked index facts
and raw root-leaf ownership have a separate INSERT entry; existing UPDATE and
DELETE admission remain unchanged.

The qualified source family has a manual Int32 primary key and nullable Short
Binary ascending/descending secondaries, each with an exact logical alias.
Aliases share one physical change. Original histories, ROW references, inactive
index tails, properties, maps, other records and EOF remain exact. New DATA and
key bytes must fit existing owned pages. Binary NODEs, splits, index allocation,
Binary primary keys and broader declaration/key combinations remain outside
this observed save profile.

NULL has its own marker; an empty Binary value has only the present marker.
Eight-byte payload segments have explicit continuation/final tags and padding.
Descending intermediate tag 09 remains unflipped. Physical cursor order is
therefore interpreted by the native profile, including empty-value ordering.
The existing constrained-read selector handles empty values conservatively;
this addition does not change read/query comparison policy.

## Planning and recovery

The existing bounded binding and fitting repacker retain original key bytes and
insert exact typed Binary/NULL keys. This path cannot select NODE continuation or
split operations. Independent proof separately reads original values, reconstructs
component/ROW streams, exact bitmap/free/history changes and complete DATA images,
then compares the whole candidate. It consumes no producer patches or key cache.

Recovery binds the original complete rows and BEFORE digest, freshly captures
native source, replans and compares both full images before effects. It retains
pending rollback, committed-mixed-target refusal and write-denied completion.
No new intent family, serialized permission or caller-selected physical mapping
is introduced.

The SQL/CLI positive profile supplies NULL for Binary. Exact empty/nonempty byte
values use the existing typed native or adapter API; no Binary literal syntax is
invented for SQL. Stale handles refuse after successful INSERT, and the prior
UPDATE/DELETE limits remain explicit.

## Evidence

The [independent source phase](../../testing/ACE16_BINARY_INDEX_SOURCES.md) passed
18 native views and 144 exact SDK observations before matching producers.
Sixteen native literal commits, independent direct proofs, recovery/forgery and
full public budget/cancellation controls pass. Typed/SQL sessions and CLI saves
match complete images and retain leases, receipts and namespace cleanup.

All sixteen prior profile regression commands pass. Strict all-feature,
journal-only, preview-only and minimal checks pass; the combined batch passes
1,153 ACE units, 79 adapter units, 102 CLI units and 90 default recovery functions.
The [execution record](../../testing/results/ace16-binary-index-insert-writes-2026-11-01.json)
retains exact receipts and the first negative-fixture, trait-method and lint
failures. No expectation was retuned to producer output.
