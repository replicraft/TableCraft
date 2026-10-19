# Complete native image retention

`NativeImage::capture` retains the entire stable/offline input on the qualified
Jet 4 and ACE code-2/5/6 read profiles
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

Duplicate known payload-column names now fail as malformed, even when their
first occurrence is NULL. Ambiguous declarations cannot hide a second payload
from inspection or the existing offline Jet 4 write gate.

## Bounded opaque catalog payload bytes

`NativeImage::read_object_payloads` returns an owned `ObjectPayloadData` for one
exact captured identity. `OpaquePayload` keeps Present/Absent/Uninspected separate
from its optional assembled binary bytes. Valid empty inline storage returns
Present with an empty byte slice; NULL and undeclared columns return no bytes with
different presence states. A physically present field with an empty definition
is rejected instead of being silently converted into NULL or empty data.

Present fields require qualified native OLE/long-value storage. The reader follows
inline, external-row or bounded chains using the existing strict long-value codec,
with a 65,536-byte limit per payload and 128 chunks. Unknown carriers, flags,
malformed lengths/references/chains, ambiguous metadata, cancellation or exhausted
budgets return no partial set. Native column order is explicitly mapped into
body/module/extra order. Record geometry, fallible allocation and shared budgets
remain enforced; unrelated property payloads are not decoded. The returned bytes
are assembled stream contents; the complete captured image separately retains
descriptors, pages, padding and every opaque object representation.

No OLE/application container, VBA source/p-code, macro or dependency semantics are
interpreted. No module or application content executes. These objects supply no
mutation handle or permission to save. All ACE writes remain blocked.

The original [payload recipe](testing/ObjectPayloadFixture.java) places inert
binary patterns in catalog fields of fresh V2016/V2019 table objects; these are
not valid VBA or native application bodies. [Independent comparison](testing/compare_object_payloads.py)
matches all bounded catalog records (37 and 41), including 2,304/12,288-byte
payloads, all byte values, NULL and empty data. A separate 65,537-byte payload per
profile fails without stdout. The registered Access example additionally matches
all 146 catalog records and 24,693 payload bytes. All input hashes stay unchanged.

The example's twelve module declarations have NULL in all three catalog fields,
yet the database contains VBA declarations. This directly reinforces that NULL
catalog payloads do not establish VBA absence or discover its actual storage.
This work does not qualify VBA extraction or application save/reopen through
Microsoft Access. The existing full-image retention remains necessary.

Synthetic CI covers field order, all byte values, empty/NULL/undeclared states,
foreign identities, inline/external/chained storage, unknown/late malformed
payloads, duplicate metadata/write blocking, budgets and late cancellation.
The engineering harness uses clap Parser and publishes tagged raw output only
after its complete object read succeeds:

```sh
cargo build -p tablecraft-ace --example object_payloads --locked
java -cp path/to/jackcess-5.0.4.jar docs/testing/ObjectPayloadFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/ObjectPayloadFixture.java new-2019.accdb V2019
python3 docs/testing/compare_object_payloads.py --jar path/to/jackcess-5.0.4.jar \
  --example target/debug/examples/object_payloads --ace2016 new-2016.accdb \
  --ace2019 new-2019.accdb --original docs/examples/Access_Example_VBA.accdb
```

## Direct catalog ownership

`inspect_object_children` reports direct captured catalog containment for a known
scoped root. Unknown/foreign roots fail instead of appearing dependency-free.
Child bodies and indirect references remain uninspected; no complete event,
startup, ribbon or VBA dependency graph follows. Existing owned children block
offline Jet 4 edits even if a table's local payloads are NULL. These checks do not
relax ACE/data-macro or native application qualification gates.
