# Byte/Int16 literal UPDATE source states

Two retained ACE header-family originals supply six read/source contexts: each
unchanged original plus two in-memory variants. No output database, database application/SDK constructor, write producer or
external reader runs during this gate. The existing independent raw fixture
model supplies expected semantic rows.

The endpoint variant changes only the third record's half-open payload span
`2..5`: Byte 255 and Int16 -32768 become Byte 0 and Int16 32767. The NULL variant
changes the first presence-mask byte from `ff` to `f9`, preserving the second
mask byte `03` and all inactive payload bytes `ff 00 80`. Complete complement
checks preserve every other byte. Full reads retain all ten fields, three rows,
original ROW references and zero indexes.

All six states pass both actual update-program absence capture and canonical
ordinary Source capture. This establishes the stated portable read/admission
observations. The four variants have independent literal expectations; they have
no separate SDK/native Access interpretation evidence. These results do not
qualify native Byte/Int16 mutation, intent decoding, recovery or saves.

One ignored engine function and scoped ACE/testkit all-target/all-feature strict
checks pass. The
[execution record](results/ace16-byte-int16-update-sources-2026-11-03.json) retains
source pins, the command and separate admission observations. Native Access
save/reopen and Windows mixed-client comparisons remain unrun.


The later [normal Byte/Int16 continuation](../access/writes/NORMAL_BYTE_INT16_FIXED_UPDATE.md)
has separate matching write, independent reconstruction, recovery and host
acceptance. Those results do not turn the in-memory variants into SDK/native
Access qualification.
