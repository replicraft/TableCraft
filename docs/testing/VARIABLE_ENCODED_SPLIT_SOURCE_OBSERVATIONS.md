# Variable DATA and encoded-index pressure sources

Two fresh ACE 16 independent-writer originals passed the five-command source
phase on its first attempt. Each contains 2,701 rows, including General Legacy,
General 2010 and Short Binary targets with measured DATA-page and index pressure.
No public UPDATE or native candidate belongs to this source phase. These inputs
do not establish combined mutation support or Microsoft Access qualification.

## Actual source profile

Each target contains 900 manual LONG IDs, its variable `KeyValue`, LONG
`Marker = ID * 17`, a fixed GUID and ordinary BYTE companions. Text targets add
four BYTE columns; Binary adds eight. The fixed GUID is
`{00112233-4455-6677-8899-AABBCCDDEEFF}`; companion `PadJ` is
`(ID + 17 * J) modulo 256`. These are stored fixed fields, not Boolean mask bits
or AutoNumber values. The proof checks their actual native descriptors, offsets
and bytes, including mixed-endian GUID serialization.

`TextLegacy` and `TextModern` retain raw, uncompressed `aaa` in declared TEXT255,
with sort descriptors `1033:0:0` and `1033:0:1`. `BinaryOne` retains one-byte `00`
in declared BINARY255. Selected flags are `2`, extended flags `0`, variable
index `0`. Every target has an ID primary, true physical Ascending/Twin aliases,
and a distinct Descending index; selected ordinary index flags remain `0x80`.
The unchanged one-row `LongSentinel` has an ID primary, raw Memo U+0100 repeated
33 times and OLE `00ff41`.

Each original has four user tables, 26 captured user/global map roles, 13 logical
indexes and 10 physical indexes. The six affected physical roots are leaves.
These 26 roles do **not** establish the complete catalog/current/historical map
census or native write admission required by a future combined consumer.

| Profile | Bytes | Pages | SHA-256 |
| --- | ---: | ---: | --- |
| V2016 | 610,304 | 149 | `2e7ca607356bf41af05d3fad6f94db9d8c78673478f64e24e941ac666114e994` |
| V2019 | 684,032 | 167 | `c5959e39bda1828d53891542210eba9b40422e0bd6b7f4023114ecc1b0dd6484` |

## Raw record and DATA capacity

All 5,400 target records across both originals are exactly 43 bytes. Every target
owns ten DATA pages with 90 ordinary roots and 32 free bytes on each page. The
actual candidate-hint membership contains its highest owned DATA page. Logical
root and final physical target references are captured separately and agree in
these fresh sources; no pre-existing forwarding or hidden records are assumed.

Text has eight columns, one mask byte, fixed prefix 30 and seven footer bytes.
Its six-byte variable stream is record range `[30,36)`. Binary has twelve columns,
two mask bytes, fixed prefix 34 and eight footer bytes; its one-byte stream is
`[34,35)`. ID starts at offset 2, Marker at 6, GUID at 10, and BYTE companions at
26. All masks, footer offsets, directory bounds and actual column fixed offsets
are independently checked rather than borrowing the preceding fixture's layout.

The Java helper projects BYTE through `Number` and unsigned normalization. Raw
bytes and metadata independently agree with those values; this is not a claim
that a particular concrete Java carrier class was validated.

The requested future changes remain `aaa` to `z` repeated 255 times and `00` to
`ff` repeated 255 times, on ID 1 only. Completed raw records would be 547 Text or
297 Binary bytes, plus a two-byte new slot. They exceed the measured 32-byte
free space on every old DATA page. Root allocations are not reclaimed during
forwarding. This establishes source pressure; no changed record or destination
has yet been written in this experiment.

## Actual encoder output and index capacity

The inert getter invokes the independent writer's whole-entry `createEntryBytes`
for both old and requested values. It therefore includes Modern entry truncation
and its digest, rather than only character-weight output. Both profiles matched
the independent finite ASCII/Binary and bit-serial digest model.

| Family/direction | Old/new key bytes | Changed root bytes | Mixed 450-entry half bytes |
| --- | ---: | ---: | ---: |
| Legacy ASC | 6 / 258 | 8,353 | 4,303 |
| Legacy DESC | 7 / 259 | 9,253 | 4,753 |
| Modern ASC | 9 / 510 | 10,403 | 5,453 |
| Modern DESC | 10 / 510 | 11,302 | 5,902 |
| Binary ASC | 10 / 289 | 11,980 | 6,130 |
| Binary DESC | 10 / 289 | 11,980 | 6,130 |

Modern requested digest tails are ASC `c1ae` and DESC `ce3d`. Original persisted
keys and ROW streams match their old encoder output throughout each source tree.
**Persisted changed keys remain unmeasured** until a separately authorized public
reference action. No changed key was inserted merely to inspect encoder output.

Original root payloads are 3,606–3,610 bytes, below capacity 3,616. Every requested
full root and one mixed half exceed that capacity. These figures use the measured
public key-only prefix policy, retaining ROW bytes per entry. They do not choose
future public partitions or an engineering full-key/ROW compression oracle.
The earlier same-width `aaa` to `zzz` results must not supply new max255 byte
goldens. In particular, a hypothetical Modern DESC mixed 225-entry leaf needs
recompression even though its ASC counterpart fits raw; neither is an observed
new partition.

## Execution and preserved preparation history

The source phase ran once: one successful javac, then create and inert inspect
for V2016, then create and inert inspect for V2019. All five commands succeeded;
there was no failed compile, retry, regeneration, size change or selector change
in this phase. The preceding independent same-width source history remains in
its [separate record](ENCODED_INDEX_SPLIT_SOURCE_OBSERVATIONS.md).

Before execution, review corrected one private DESIGN paragraph: an ordinary
root's retained range is zeroed before the pointer; an already-forwarded stub's
remainder and abandoned hidden bodies stay preserved under the existing
engineering policy. Exact initial DESIGN, freeze, handoff and static result were
archived. Only that prose input changed; helper, model, recipe, oracle and all
five argv stayed identical. The corrected freeze, not its unexecuted predecessor,
binds this source run. No geometry or action expectation was repaired afterward.

Each child had a 30-second deadline and 512 MiB heap cap, with an 8 MiB writable
file ceiling and live 16 MiB bound per captured stream. Expressions were disabled,
foreign keys enabled, free-page reuse disabled, UTC selected and linked opens
refused. All 28 input/freeze checks and 17 created artifact pins stayed unchanged.
A separate saved postcheck rederived both complete source snapshots without a
compiler, JVM or native call.

The [repository-authored helper](variable_encoded_split_sources/VariableEncodedProbe.java)
is byte-identical to executed source. The maintained
[model](variable_encoded_split_sources/model.py) changes only repository-root
composition; all eight function ASTs match. The
[saved verifier](variable_encoded_split_sources/verify.py) checks source and archived
preparation identities, 5,402 typed rows, 5,400 target slots, 60 target DATA pages
and 21,602 logical index entries including aliases, with zero fixture processes.
The reviewed Jackcess 5.0.4 JAR/source provenance follows the
[fixture ledger](FIXTURES.md); no upstream fixture database is copied.

```sh
python3 -B docs/testing/variable_encoded_split_sources/verify.py
```

The [compact receipt](results/variable-encoded-split-source-observations-2026-10-27.json)
pins the executed sources, actual report, original bytes, archived prose correction
and saved checks. Later public references, old-variable baselines and combined
engineering candidates require separate records and authorization. This source
qualification does not grant map growth, arbitrary page reuse, tuples, unique
key overflow, French/unknown sorts, macros, normal saves or native Access parity.
