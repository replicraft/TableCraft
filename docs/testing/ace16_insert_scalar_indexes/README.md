# Scalar secondary INSERT evidence

Sixteen fresh V2016/V2019 originals cover Boolean, Single, Double and legacy
DateTime secondary indexes in ASC and DESC order. Each table has manual required
Int32 ID, Group and Short Binary Bytes. Four logical index names share exactly
two physical owners: the required unique ASC primary and ordinary nonunique
secondary. These originals do not qualify secondary uniqueness or IgnoreNULL.

All sixteen complete engineering images were derived from original bytes and
closed requests, then frozen before the committed producer build. Sixteen
producer calls matched those images on their first attempts; only then did
thirty-two fresh read-only getter/map calls pass. Across the cases, 3,840 original
ROW references and bodies remain exact, with 24 inserted rows, 3,864 final rows
and 7,728 physical key/ROW entries.

Boolean cases insert duplicate false/true values while preserving the secondary
historical distinct count. False is a mask-clear Boolean value, not NULL. The
actual declaration is length one while physical Boolean payload width is zero.
Single cases preserve finite source bits and insert distinct negative and
positive zero. Double cases insert negative zero. DateTime sources use exact
positive day-plus-half serials and insert the epoch's positive zero under explicit
UTC/LOCAL_DATE_TIME. Raw fields, runtime getter projections and actual public
createEntryBytes output are checked independently. NaN, infinity and negative or
adjacent-ULP DATE transport are not public-writer observations from this corpus.

Boolean images stay fitted. The twelve Single/Double/DateTime cases each append
two INDEX pages, for 24 INDEX pages total; no DATA or bitmap page is added. Primary
maintenance and DATA placement remain part of every complete-image comparison.
Historical distinct counters count key-only families, including ordinary NULL
once, rather than complete ROW populations.

The independent public-writer experiment completed all 97 children on its first
attempt: one compilation, sixteen generations, thirty-two before reads, sixteen
fresh-copy appends and thirty-two after reads. All sixteen raw source gates
passed before any append. Public framing, partitions and scratch are separate
observations; no public AFTER image supplies engineering expected bytes.

An earlier pure static check failed because the draft treated
IndexData.createEntryBytes as public. The pinned JAR declares it package-private.
The corrected repository-owned probe uses the same reflected declared-method
seam as existing encoded-key probes, with the Object-array argument preserved.
That failed static source/check/freeze/error is retained privately. No javac,
Java source generation or database mutation occurred before this correction;
the actual 97-call and 16+32-call runs remain first-attempt successes.

Run the maintained saved proof:

```sh
python3 -B docs/testing/ace16_insert_scalar_indexes/verify.py
```

The default executes no child process. `--verify-git` explicitly adds one Git
metadata batch checking 4,462 exported source files and the committed build
receipt; it never runs a fixture. `--evidence-root PATH` selects an explicit
artifact root. Missing private evidence reports `available: false` and exits 2,
without regeneration or fallback.

The proof requalifies every original before reconstructing all expected images,
then checks public references separately. It compares all sixteen native images
before interpreting saved fresh getters and checks exact command inventories,
bounds, streams, typed bits, ROWs, trees, counters, maps and protected bytes.
Historical absolute argv remain immutable metadata; this package reconstructs
that recorded execution scope without embedding machine-specific paths.

`reference.py` preserves the executed functions with only the repository bootstrap
changed. `model.py` preserves all constructor functions except the private import
loader, using the maintained frame owner directly. The Java helper is the exact
repository-owned source compiled in the successful experiment. No third-party
implementation or database binary is copied into this package.

See the [compact evidence record](../results/ace16-insert-scalar-index-native-2026-10-28.json)
and [committed producer build](../results/ace16-insert-scalar-index-example-build-2026-10-28.json).
These are engineering-candidate and independent-reader observations. They do not
qualify production saves, Microsoft Access save/reopen, data macros or Windows
integration, nor do index bytes establish SQL predicate coercion semantics.
