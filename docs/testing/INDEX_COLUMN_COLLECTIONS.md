# Native index metadata collection experiment

The development-only `index_columns` benchmark compares the current Vec against
ArrayVec with ten inline columns and SmallVec with two inline columns. The native
ten-column format ceiling is unchanged. This harness does not select a production
container or change the working engine.

`IndexColumnsFixture.java` creates fresh ACE 16 and Jet 4 inputs with one, two and
ten key components and one or twenty-four indexes per table. Existing read-only
IndexOracle getters provide independent descriptors. Keep inputs unchanged;
retain generated files and build copies under ignored `.cache/` directories.

`prepare_index_column_variants.py --directory PATH` requires a fresh directory
and copies only ACE into three isolated workspaces. Other local dependencies
remain read-only references to the checkout. Build each workspace's
`index_columns` bench with `cargo build --release --bench index_columns --offline`
and retain the executables as `index-columns-vec`, `index-columns-arrayvec` and
`index-columns-smallvec` in the experiment directory. Refresh the isolated lock
files offline if needed; do not update registry versions. The script does not
compile, time or mutate the working engine.

The verifier expects `fixtures/index-columns-V2016.accdb` and
`fixtures/index-columns-V2000.mdb`, plus `V2016.full-oracle.tsv` and
`V2000.full-oracle.tsv` from IndexOracle. Run
`python3 docs/testing/verify_index_column_variants.py --directory PATH` before
timing. It checks all three complete descriptor sets, key checksums, retained
original/binary hashes and isolated constructor allocation accounting.

Each executable provides `describe`, `inspect`, `vec`, `arrayvec` and `smallvec`
workloads with explicit `--database`, `--table` and `--iterations`. `inspect`
measures complete public native metadata inspection, including ordinary budgets;
the container workloads isolate construction/iteration. Input file admission is
capped at 64 MiB. Container allocation counts are derived from capacity/spills,
not observations from a global allocator. Object sizes and retained heap storage
must be reported alongside timings: removing an allocation can enlarge every
index object.

Keep timing windows free of competing builds, tests and benchmarks. Preserve all
raw samples, including ties/noisy runs, and fixed source/binary/input hashes.
Neither this procedure nor getter agreement establishes a speedup, production
adoption, native SQL compatibility or Windows Access qualification.
