# Shared engineering fixtures

`tablecraft-testkit`, under `crates/engineering/`, owns working independent raw
Jet/ACE fixture builders used by ACE, Access and CLI tests. Its `access` module
builds literal catalog records, DATA pages and small database images, and writes
little-endian test words. The crate has no dependencies and imports no production
parser, encoder, format constants, engine or adapter.

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
