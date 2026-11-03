# Shared test fixtures and support

`tablecraft-testkit`, under `crates/engineering/tablecraft-testkit`, owns shared
fixture data and test machinery. Workspace consumers use it through
`dev-dependencies`; production engines, adapters and applications do not acquire
it as a normal dependency. The crate has no application-package dependency.

## Ownership

```text
src/
├── fixtures/
│   ├── access/                 independent images, requests and literal pins
│   │   ├── tcna2.rs            independent original-presence wire grammar
│   │   └── original_fixed_wire.rs literal TCNI4/8 original group requests
│   └── formats/deployment.rs   independent raw CAB construction
└── support/
    ├── ace/sentinel.rs         shared native read observations
    ├── ace/page_reads.rs       byte-exact read-position counter for six consumers
    ├── ace/mutable_source.rs   shared adversarial source for lifecycle observations
    ├── ace/resources.rs        public planner budget/cancellation assertions
    ├── query/multivalue.rs     query Source, DTO and assertion machinery
    ├── access/owned_growth.rs  retained adapter-session workflows
    ├── cli/mod.rs              headless process runner with explicit executable
    └── cli/owned_growth.rs     process, save and readback workflows
```

Import the actual owner directly, for example
`tablecraft_testkit::fixtures::access::genuine_owned_growth` or
`tablecraft_testkit::support::access::owned_growth`. Do not leave forwarding
modules or reexports at the former engine, adapter or CLI helper locations.
New shared machinery belongs in the corresponding testkit folder; a helper used
by only one test can remain with that test until sharing is useful.

`support::ace::resources::assert_public_boundary` measures each caller's actual
public planner, then checks exact budgets, each one-less budget and entry/middle/
terminal cancellation. Callers retain input construction, typed error projection
and fixture-specific result assertions. The helper does not substitute internal
cost formulas. `fixtures::access::tcna2::original_presence_wire` shares only the
independent literal test grammar; callers retain their original rows, source
bindings and exact expected frame lengths. Neither helper imports a production
encoder or turns a fixture into write permission.

`support::ace::page_reads::PageReads` owns the shared seekable read counter used
by native catalog/row/page/projection and adapter browsing tests. It records actual
read starts, so callers can assert that unselected pages were never read. The
helper moved byte-for-byte from engine test support; six existing targets now
import this owner directly. Access tests enable the existing `ace-support` dev
feature for these consumers. The default testkit and production dependencies
remain unchanged. Its [acceptance record](results/shared-page-read-support-2026-11-02.json)
covers all six consumers, strict checks, minimal features and dependency guards.

`fixtures::access::original_fixed_wire` owns literal TCNI4/8 group framing.
Its single-column numeric helper preserves the original Int32/Int64/Currency/NULL
contract; the IEEE fixture consumes the same envelope with independent raw tags
and canonical padding. Six old-path consumers import the real owner directly;
the old module is removed without a shim. Table/column names, original bindings,
requests and image expectations remain with each fixture. No engine serializer
is imported.

Two full literal goldens, six actual IEEE native/legacy-boundary functions and
five retained numeric commit functions pass. The latter cover Marker, generated
Score, Binary Id, wider numeric siblings and ignored-NULL originals. Scoped
strict and minimal testkit checks also pass; see the
[extraction record](results/shared-fixed-wire-fixtures-2026-11-03.json).
The [earlier TCNI4 record](results/shared-tcni4-literal-framing-2026-11-02.json)
remains historical evidence.

The independent `fixtures::access::{complex_schema, attachments,
attachment_modern}` modules own the existing raw collection builders. Engine,
adapter and CLI tests import those owners directly. Their move preserves builder
bodies and literal bytes; no production codec or dependency is introduced. Public
and restricted test-module consumers retain their actual child dependencies.
The [extraction record](results/shared-complex-attachment-fixtures-2026-11-02.json)
retains the complete-consumer correction, lint cleanup and 435 passing runtime
functions across engine, adapter, CLI and private preservation checks.

`fixtures::access::version_history` owns the shared independent stored-history
builder. Six existing engine/adapter/CLI consumers import it directly; Text and
DATE-bit inputs remain exact. The
[extraction record](results/shared-history-fixture-2026-11-02.json) preserves the
lint cleanup and 69 passing runtime functions, with strict and minimal checks.

## Optional subsystem support

The default feature set is empty. Raw fixtures depend only on
`tablecraft-core` carriers and the existing UUID byte-order utility. They do not
call production encoders or readers to
produce their expected bytes. Tests opt into the support they actually use:

| Feature | Module | Working boundary |
| --- | --- | --- |
| `formats-fixtures` | `fixtures::formats` | Independent compressed and uncompressed CAB construction |
| `ace-support` | `support::ace` | Native engine inspection |
| `query-support` | `support::query` | Backend-neutral query Source and execution contracts |
| `access-support` | `support::access` | Real Access adapter, offline session and filesystem contracts |
| `cli-support` | `support::cli` | Executable process, filesystem and output observations |

The Access support module is available on macOS and Linux, matching its retained
offline-session test boundary. Optional support dependencies belong to the
engineering testkit and are enabled for test consumers. A consumer may forward
its matching capability feature to the dev-dependency support feature; that does
not make the testkit a production dependency.

Each CLI integration target configures a testkit `Runner` once with
`Runner::new(env!("CARGO_BIN_EXE_tablecraft"))` and calls `CLI.run(args)`.
The headless runner removes `DISPLAY` and `WAYLAND_DISPLAY` before launching the
process. The owned-growth workflow accepts an explicit `&Path` executable at
its calling test. The dependency crate neither resolves the Cargo executable
variable nor imports the CLI application. Source images, expected images and marker values
also remain explicit inputs. Adapter workflows similarly receive the caller's
catalog scope rather than assuming a shared identity.

## Preserve independent evidence

Sharing transport, callbacks, leases and readback loops does not merge the
contracts exercised by native proof, adapter sessions and CLI publication. Keep
named tests, fixture families, both header variants, resource and cancellation
controls, receipts, exact output and namespace assertions at their applicable
boundaries.

Literal raw records, index keys, journal wires, whole-image expectations and
source hashes remain independent of production serializers and mutation code.
A shared pin loader can bind the same immutable fixture at several boundaries;
it cannot replace an independent byte replay or establish write permission.
Keep source qualification, constraint-negative exclusions and mutation outcomes
explicit when consolidating metadata. Never generate the expected side of a test
with the producer it checks.

See the [duplication audit](TEST_DUPLICATION_AUDIT.md) for the consolidation scope
and the [fixture policy](FIXTURES.md) for provenance and qualification requirements.

## Historical extraction evidence

Earlier receipts retain the paths and hashes from their recorded revisions.
The current fixture namespace is `fixtures::access`; historical evidence is not
rewritten by this organization change.

### Extraction check

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

### Python artifact reuse

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

### Secondary-index source owners (30 October 2026)

`fixtures::access::secondary_index` supplies the actual authored BEFORE and complete
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

The same stored-history fixture owner now includes `version_history::mixed`,
the existing independent two-history builder moved from its Access test owner.
Engine freshness controls and the original query-source tests import it directly;
current-Memo descriptor overlays also serve engine and browser observations.
See the [reader acceptance record](results/lazy-history-reader-2026-11-02.json).

`fixtures::access::genuine_wide_ordinary_siblings` owns the eighteen independently
qualified source pins and original requests used by native proof/recovery, adapter
and CLI tests. It shares the existing literal TCNI4 grammar without importing an
engine encoder. See the [write acceptance record](results/ace16-wide-sibling-writes-2026-11-02.json).

`fixtures::access::collection_scale` owns retained corpus pins and independent
recipe expectations; `support::ace::collection_scale` owns shared real-reader
limits/observations. Engine and browser scale targets import these owners directly.
The [scale acceptance record](results/lazy-collection-scale-2026-11-02.json) separates
recipe checks, full-reader parity and cumulative resource controls.
