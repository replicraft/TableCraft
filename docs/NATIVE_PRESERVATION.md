# Complete native image retention

`NativeImage::capture` retains the entire stable/offline Jet 4 or ACE 2007 input
before decoding its catalog. Read cursors borrow immutable bytes; there is no
mutable byte accessor. Every opaque object, VBA/project body, unknown catalog
record, unused page and padding byte is retained without interpretation. Capture
checks the observed length/header, but callers must exclude noncooperating writes.
It is bounded by the native 64 MiB ceiling and shared read/allocation/work budgets;
cancellation is checked between 64 KiB chunks. No active content executes.

`dependencies()` exposes conservative catalog evidence: module declarations report
execution unavailable; forms/reports/macros report unavailable application behavior;
other objects remain Uninspected. These statuses neither discover individual VBA
references nor establish absence of data macros. Unknown coverage is never permission
to write. No complete event/button/startup/ribbon dependency graph is implemented.

Tests retain all 3,387,392 original example bytes and 146 declarations, inspect its
38-row Roulette table from a borrowed cursor, and report module/application coverage.
A synthetic unknown kind and an unused page full of marker bytes survive exactly.
Budget and midstream cancellation failures return no image. These are capture/read
results; unrelated edit/save/reopen preservation and native Access comparison still
require separate evidence. [VBA scope](VBA_COMPATIBILITY.md) remains unchanged.

[Fixed-field edit plans](NATIVE_WRITES.md) now compare all bytes outside validated
patches. This qualifies a bounded byte-preservation mechanism on synthetic/Jet 4
engineering fixtures, while actual ACE/VBA application save/reopen remains blocked.

## Inert payload presence

`inspect_object_payloads` checks native Lv/LvModule/LvExtra null masks for one
catalog-scoped identity. It never follows or decodes those payload pointers.
Present, Absent and Uninspected remain distinct; older rows lacking an appended
column contain NULL, while a missing catalog column is Uninspected. These are
physical presence hints, not a complete VBA/data-macro dependency graph.

The Jet 4 edit gate requires all three payloads to be explicitly absent on the
selected table. Opaque or missing coverage blocks writes. Tests demonstrate an
extra-payload hint blocks editing without following its deliberately unqualified
body, while foreign identities/cancellation fail and absent catalog columns remain
Uninspected. ACE write blocking remains unchanged.
