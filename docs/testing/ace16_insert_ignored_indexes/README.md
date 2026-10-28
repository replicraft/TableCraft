# Ignored-NULL secondary INSERT evidence

Eight repository-generated V2016/V2019 originals qualify ten INSERT cases. The
same immutable empty-secondary original feeds two separate copies per profile.
Each table has a manual required Int32 primary and a nullable Int32 secondary,
with four logical names sharing two physical indexes. The secondary is ASC,
nonunique, IgnoreNULL and not Required; the Group field itself is nullable.
Unique and DESC variants have separate literal Rust tests, not public-writer
qualification in this corpus.

All ten complete engineering images were derived from original bytes and closed
requests, then frozen before the committed producer build. Ten producer calls
matched those images on their first attempts. Only afterward, twenty fresh
read-only getter/map calls passed against the model-derived ROWs, typed values,
raw records, trees, counters, maps and protected original bytes. The ten cases
contain 1,950 final rows and 3,868 physical key/ROW entries; 1,818 original ROWs
are preserved across cases, with 132 inserted rows. Reused originals explain why
case totals differ from the eight-origin inventory.

The all-NULL cases preserve the complete secondary LEAF or internal tree,
secondary membership map and historical distinct counter. First non-NULL adds
one family to an empty secondary; mixed requests add one new family despite
repeated and omitted keys. Pressure inserts 56 rows, of which 54 contribute new
secondary families; one DATA page precedes four INDEX pages in each profile.
Historical counts are not treated as live row counts.

The independent public-writer experiment retains its actual two-report lineage.
The first controller stopped after two successful children because Jackcess
emitted an omitted-NULL warning and the controller required empty stderr. The
remaining 53 commands ran once in a separate continuation. All eight raw source
gates passed before any of ten reference appends. No completed child repeated.
The warning is explained by the pinned source JAR's IndexData.prepareAddRow
667–671 and commitAddRow 730–743: ignored all-NULL keys leave the pending entry
null, and commit logs that no-op. The continuation admits only exact paired JUL
headers/messages, database/table/index context and counts derived from source or
request NULLs; all raw stderr remains retained.

Run the maintained saved-only proof:

```sh
python3 -B docs/testing/ace16_insert_ignored_indexes/verify.py
```

The default executes no child processes. `--verify-git` explicitly adds one Git
metadata batch checking all 4,449 exported source files and the committed build
receipt; it never runs a fixture. `--evidence-root PATH` selects an explicit
saved-artifact root. Missing ignored evidence returns `available: false` and
status 2, with no regeneration or fallback.

The verifier first requalifies all eight original states and reconstructs all ten
expected images. It then replays the independent public references, checks all
ten native images before interpreting cached fresh getters, and verifies raw
command inventories, bounds, stream pins and complete input closures. Historical
absolute argv are interpreted as recorded metadata; maintained paths and this
package's provenance record are repository-relative.

`reference.py` preserves the executed raw proof functions with only the repository
bootstrap changed. `model.py` preserves every constructor function except the
private dynamic-import loader, using direct maintained owners instead.
`omission_warnings.py` owns the two exact warning checks from the executed
continuation. `IgnoredIndexInsertReference.java` is the exact repository-owned
helper used in the first compilation. No third-party implementation or database
binary is copied into this package.

The first maintained verifier run failed during import because its original
`warnings.py` name shadowed Python's standard-library module. The initial files
and error are retained privately; renaming the owner to `omission_warnings.py`
fixed that packaging error without changing executed tools, models, originals,
expected images or actual outcomes. The corrected default replay and explicit
Git/missing-evidence contracts pass separately.

See the [candidate record](../results/ace16-insert-ignored-index-native-2026-10-28.json)
and [committed build](../results/ace16-insert-ignored-index-example-build-2026-10-28.json).
These are engineering candidate and independent-reader observations. They do not
qualify normal native saves, Microsoft Access save/reopen, data macros or Windows
integration. Public-writer framing and scratch remain separate from the complete
engineering byte policy.
