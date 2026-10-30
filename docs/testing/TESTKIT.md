# Shared engineering fixtures

`tablecraft-testkit`, under `crates/engineering/`, owns working independent raw
Jet/ACE fixture builders used by ACE, Access and CLI tests. Its `access` module
builds literal catalog records, DATA pages and small database images, and writes
little-endian test words. Its core-values dependency supplies the actual shared
literal-row `Value` conversion; it imports no production parser, encoder, format
constants, engine or adapter.

All three consumers declare it under `dev-dependencies`. The dependency guard
rejects a non-development dependency on this crate. Tests import the builders
directly from `tablecraft_testkit::access`; the former ACE test helper is removed,
with no compatibility re-export. Production code cannot obtain fixtures through
the normal dependency graph.

## Extraction check

The [27 October receipt](results/testkit-builder-equivalence-2026-10-27.json)
records a standalone comparison against the committed former ACE helper. After
removing line comments and whitespace, the old and moved sources have identical
token sequences. A separate standard-library-only Rust witness compiled and ran
successfully on its first attempt, comparing complete byte buffers:

- All four combinations of `image(overflow, indirect)`, each 28,672 bytes.
- Three catalog records, including empty names and signed integer boundaries.
- Nine DATA pages, including empty pages, explicit slot flags and an empty deleted
  slot body.
- One buffer containing little-endian word and integer writes.

All 17 comparisons matched exactly. Exact old/new sources, witness, executable,
four generated images and runtime log remain in the ignored evidence directory
named by the receipt. The check used `rustc`, without Cargo or native/JVM fixture
calls. It establishes extraction equivalence for these builders and inputs; it
does not establish native Access behavior or merge independent compatibility
proofs into one qualification.

## Python artifact reuse

`scripts/compat_artifacts.py` separately provides the working
`ArtifactStore(root).path`, `.pin` and `.load` operations used directly by the
maintained ordinary-hint candidate and reference verifiers. It enforces resolved
root containment and computes fresh byte lengths and SHA-256 hashes; JSON parsing
retains standard-library behavior. Protocol-specific models, selectors and proof
checks remain in each verifier.

Two filesystem unit tests cover containment, symlink escapes, changed/missing
files and JSON loading. Both maintained saved-result verifiers passed after the
refactor, without native/JVM fixture calls. Their previous maintained versions
were archived before edits; historical receipt pins still describe those exact
earlier versions. There is no Rust compatibility crate or wrapper API for this
Python reuse.

## Secondary-index source owners (30 October 2026)

`access::secondary_index` supplies the actual authored BEFORE and complete
expected creation images to native and host tests. Its pure seed, definition/map
and KEY/ROW framing dependencies also moved into the testkit. Existing ACE INSERT
expected algorithms remain in ACE with direct imports; there are no forwarding
fixture owners or copied source graphs. The core-values dependency serves the
real shared `Row::values` conversion.

All 31 moved function bodies and five retained INSERT algorithm bodies remain
identical. The first identity command failed at compilation before constructing
images: 121 missed inherited, local and path-loaded consumer bindings. Direct
owner imports corrected those bindings; separate host test compile errors were
corrected the same way. No source literals or expected algorithms changed.

The first actual identity execution passes: all eight BEFORE images, sixteen
complete expected images and request/row/order TSV match all 49 preserved
pre-move files exactly. The SOURCE reader/census gate also passes 16 pairs and
32 views. The native mutation regression passes 559 functions with three explicit
helpers ignored; Access and CLI units pass 67 and 51 functions respectively.
The selected CLI regression targets pass 46 integration functions. Native and
host all-target/all-feature strict checks, minimal testkit/ACE/Access builds,
dependency boundaries (15 packages), all 12 boundary tests, formatter and
documentation links pass. Strict inventories exposed further private or unused
imports, corrected without algorithm or fixture changes. The [extraction result](results/testkit-secondary-index-builder-extraction-2026-10-30.json)
retains every failed import-diagnostic phase separately from the final passes.

Historical model paths and qualification records keep their original hashes;
they are not repointed to current source owners. Exact predecessors remain in
the retained extraction archive. This source-owner refactor does not expand
write admission or qualify native Access behavior, macros or saves.
