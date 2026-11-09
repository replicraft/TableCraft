# GUID and GUID/Int32 secondary INSERT evidence

This repository-owned corpus covers 32 cases on 32 generated V2016/V2019
originals. Each table has a manual required Int32 primary ID, nullable GUID Group
and nullable Int32 Score. Four logical names share two physical indexes.

The observed secondary profiles are single GUID ASC/DESC under ordinary
nonunique pressure, and two-component GUID/Int32 in both written orders and all
four direction combinations under ordinary nonunique pressure. Fitted tuple
cases use mixed directions and ordinary, unique or unique IgnoreNULL settings.
IgnoreNULL omits only the all-NULL tuple; partial-NULL tuples remain indexed.
This corpus does not qualify ten-component tuples or single-GUID unique/ignored
profiles through an independent writer.

The first public-writer experiment completed all 193 commands: one compilation,
32 generations and 64 source reads, followed only after all source gates passed
by 32 disposable-copy appends and 64 reference reads. Twelve known omitted-NULL
warning events are checked against the existing exact warning grammar; other
stderr must be empty. The original captures and protocol remain unchanged.

All 32 complete engineering images were then derived solely from qualified
SOURCE bytes and exact requests and frozen before the isolated committed-source
producer build. The first 32 producer calls matched those full images before
any of the first 64 fresh read-only getter/map calls ran. These phases preserve
2,768 old logical ROWs, insert 80 rows and contain 2,848 final rows and 5,684
physical key/ROW pairs. Public-writer framing and scratch bytes are observations,
not candidate golden authority.

Raw GUID fields use mixed little-endian storage for the first three groups;
index keys use canonical GUID bytes with independently checked delimiters and
per-component direction. The Java getter projects GUIDs as braced uppercase
String16 values, not a UUID runtime carrier. Raw zero GUID, NULL GUID, partial
NULL and all-NULL tuple identities remain distinct. Reflected createEntryBytes
outputs qualify old and requested keys separately from the engineering model.

Run the saved replay from the repository root:

```sh
python3 -B docs/testing/ace16_insert_guid_tuple_indexes/verify.py
python3 -B docs/testing/ace16_insert_guid_tuple_indexes/verify.py --verify-git
```

The default starts no child process. The explicit Git option starts one metadata
batch binding the archived compiled source and committed build receipt. Neither
mode starts a producer, Java process or fixture generator. Missing ignored
evidence returns JSON with `available: false` and exit status 2; it never creates
replacement evidence. Use `--evidence-root PATH` to select a complete relocated
evidence tree.

`recipes.py` and the Java helper retain exact private bytes. `reference.py` and
`stderr_policy.py` change only repository bootstrap and owning-module imports;
the verifier checks exact transformations and function ASTs. `model.py` replaces
its private dynamic loader with a direct import from the existing multiple-index
model owner; all constructor functions and the body from `def n` onward remain
exact. The maintained verifier checks frozen input/output pins, all source gates
before image derivation, public transitions after derivation, all candidate image
bytes before interpreting fresh getter streams, typed rows, counters, maps and
record placement. It also rechecks every observed pin before returning success.

Original databases, JARs, compiled classes, raw streams and expected images stay
in ignored private evidence. The Java fixture is repository-authored; no
third-party source or fixture is copied into this package. See the
[compact result](../results/ace16-insert-guid-tuple-index-native-2026-10-28.json)
and [fixture provenance](../FIXTURES.md).

These are engineering candidate and independent-reader observations. They do not
qualify native Microsoft Access saves/reopens, data macros, Windows integration
or normal production write paths.
