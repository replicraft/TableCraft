# Exact-slot Text/Binary index splitting

With `ace16-write-preview`,
`NativeImage::preview_ace16_split_slot_updates(table, column, requests, budget, cancelled)`
returns a separate non-journalable `Ace16WritePreview`. It combines existing exact
Short Text/Binary slot updates with recursively split single-key indexes. Existing
fixed/numeric, slot, page and variable-record entrypoints retain their contracts;
normal CLI/backend saves gain no new write path.

The selected field must satisfy the existing variable-slot storage rules. Each
non-NULL replacement must have a supported encoding occupying exactly its old
span; records, offset directories and row references never move or resize. NULL,
empty, compressed-marker and raw UTF-16 representations retain the existing slot
contract, including opaque old payload preservation when presence is cleared.
Exact expected values and original snapshot ordinals remain mandatory. Data-page
allocation and forwarding are outside this API.

Affected indexes must be checked single Short Text indexes with the existing exact
General legacy/modern descriptor, or single Short Binary indexes. True physical
aliases share one operation. Affected tuples are explicitly refused, including
empty/noop request lists; the old slot API retains its existing-fit tuple behavior.
French/unknown collations, Memo, ignore-NULL and unsupported unique/overflow
profiles remain refused through existing qualification. Full original structure,
properties, relationships, requiredness, uniqueness and final constraints still
apply. This does not qualify macros, native Access saves or arbitrary codecs.

Owned replacement keys are genuinely encoded from changed row values. Untouched
keys retain their original complete wire bytes. Both numeric and encoded consumers
use the [retained planner](ACE16_RETAINED_SPLIT_PLANNER.md), its independent capacity
and topology replay, and the same map-allocation authority. Full key plus ROW
maxima, per-height links, root allocation order, original residue and zeroed fresh
padding remain certified. New fitting-root checks compare complete decoded keys
and ROWs against request-derived expectations; matching lengths/prefixes alone
cannot authorize different bytes. The existing [composite fitting audit](ACE16_COMPOSITE_FITTING_AUDIT.md)
remains separate and unchanged in scope.

The slot finalizer rebinds immutable original table/column metadata, reruns original
slot staging and compares the exact patch inventory and changed-row count. Its
index audit keys come only from that rebound stage. Fixed and slot finalizers
retain separate request authority and constraints while sharing a narrow
exact-before append composer. That composer checks original image length, patch overlap and contiguous EOF
rules, copies only a complete candidate and proves
original-prefix preservation. It creates no journalable native edit plan.

The dedicated engineering consumer is:

```text
preview_ace16_split_slot SOURCE OUTPUT TABLE COLUMN TSV
```

Build it with `cargo build --locked --offline -p tablecraft-ace --features ace16-write-preview --example preview_ace16_split_slot`.
Its clap parser reuses the existing headerless five-field quoted TSV loader:
`ROW EXPECTED_KIND EXPECTED_DATA KIND DATA`, separated by tabs. Kinds are `null`,
`text` and `binary`; NULL requires empty data, Text is literal UTF-8 including
quoted NUL/tab/newline, and Binary uses unsigned hexadecimal. The file ceiling is
16 MiB/4096 assignments with a 1024-byte per-value input ceiling, under shared
256 MiB/100,000-row/100-million-step allowances. All requests are captured and
validated before opening the database. Publication uses a fresh `create_new`
output, synchronizes the completed candidate and removes only its own failed
partial file. It never overwrites the source or an existing output. Success prints
`candidate_changed_rows=N` and an engineering qualification notice.

The [implementation result](results/ace16-encoded-index-splits-native-2026-10-27.json)
records 142 distinct runtime functions, 15 new, plus three compile-fail doctests.
Nine independent functions cover six complete root images for both General
families/Binary and directions, NULL/empty and weight ties, Binary segment widths,
unchanged long digest wires, root/nonroot growth, aliases, shared map bytes and
promotion, constraints, guards, limits, cancellation and retry. Three private
functions reject correlated replacements, wrong/missing/extra row patches,
equal-length wrong keys, opaque index edits and unrequested EOF. The new consumer
runs nine functions; its shared publication test is counted only once across the
new and fixed consumers. Feature/default strict checks, minimal build, warnings-denied
rustdoc, example build/help and scoped formatting pass.

The first independent run preserved seven passing functions and two authored
expectation failures. One assumed an ignore-NULL index could supply an ordinary
All snapshot; complete table/check/constraint controls now retain that selected
read's actual Unsupported result. The other expected a bounded fresh RIGHT node
when the established overflowing-root policy gives both nested nodes tails. Its
six-byte correction changes no base-six image or production behavior. Test-only
compile/style errors and exact source/log histories remain pinned separately.

This chunk runs authored in-memory fixtures and filesystem consumer tests. It does
not claim original-file candidates or public/native write parity. Qualified source
and frozen old-slot baseline evidence are separate; committed-artifact original
candidate comparisons remain pending. No independent changed non-NULL long-digest
key oracle is claimed; long-key controls retain existing wires or replace with NULL.
