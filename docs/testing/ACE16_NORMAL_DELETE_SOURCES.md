# Ordinary DELETE source qualification

The [model](ordinary_delete_originals/model.py) and
[original requests](ordinary_delete_originals/requests.json) use unchanged genuine
SDK-created originals. Literal AFTERs decrement Sentinel's TD row count from one
to zero and set both native tombstone bits on its logical root. Stored record
payload, OLE, properties, maps, free/slot counts and EOF remain exact. No native
writer or prior writer output constructs the expectation.

Four native source views and four independent SDK reports pass before the
corresponding deletion producers. The native gate also checks fresh existing
Source admission after the tombstone. The explicit
[zero-row observer](preserved_fixed_scalars_genuine/OrdinaryDeleteGetter.java)
preserves the old formatter and all unrelated exact row bounds; the original
one-row observer remains unchanged. Compile with the existing bounded channel
and pinned Jackcess 5.0.4, then use `--inspect-sentinel-zero` on AFTER files.

See the [source record](results/ace16-normal-delete-sources-2026-11-01.json) for
pins and retained first failures. Generated files, classes and JARs remain outside
Git. These reads do not establish native Access deletion/save/reopen conformance.
