# Compound DATA and INDEX source qualification

Four existing full84 views now pass current native source checks before matching
normal compound-allocation producers run. The two originals and independently
modeled AFTERs are reused unchanged; this phase constructs no new expected file.

One complete row includes 84 Binary bytes and needs a 97-byte native record.
All eleven original owned DATA pages have only 23 free bytes, while the new
primary entry also overflows the root leaf. The expected images append DATA first,
then right/left INDEX pages. Original represented GLOBAL_FREE and separate
DATA-owned, hint and INDEX-owned cells provide explicit authority for all three.
No authority is inferred from zero bytes, EOF or an absent map window.

The native gate independently reconstructs complete images, records, physical
ROWs, key partitions, root residue, history and map complements. It checks actual
native17 catalog admission, the nonempty Required property, constraints and full
current Source/NoRequired qualification on each original and AFTER.

The [source record](../../results/indexes/ace16-compound-root-split-sources-2026-11-01.json)
retains pins and the first successful native receipt. This is source/read evidence;
it does not establish a working compound normal save or native Access save/reopen.
