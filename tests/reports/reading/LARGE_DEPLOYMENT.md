# Original large Access deployment corpus

The repository-authored [Java recipe](LargeDeploymentFixture.java) produces
original ACE 16-family V2016 and V2019 databases larger than 64 MiB. The original
[streaming CAB writer](build_large_deployment.py) wraps each database in stored
and MSZIP packages. No third-party database, CAB binary or writer implementation
is copied. The reviewed Jackcess 5.0.4 artifact is used through public builders
and read-only getters, with expression evaluation disabled.

Fixture generation, reopened getter checks and independent Python payload models
have passed. The new spooled input path passes 58 frozen CLI controls: 48 successful
observations and ten expected resource refusals. These observations do not qualify native Access
deployment signing, installation, active content or runtime execution.

## Original data and bounded construction

Each database contains `LargeDeployment` with 1,100 rows. `Id` and `Score` are
ordinary Int32 fields, `Label` is Text, and `Memo` is uncompressed Unicode Memo.
The authored IDs are 1 through 1,100; scores are `Id * 17 - 9`; labels are
`row-000001` through `row-001100`. Each Memo contains 32,768 deterministic ASCII
characters. Position `p` in row `id` is the character with code
`32 + (id * 17 + p * 29 + floor(p / 97)) mod 95`.

The Java writer constructs one row at a time with a 256 MiB heap ceiling. Its
reopened read-only getter verifies every actual field and row, then publishes
selected payload hashes and a concatenated UTF-8 Memo digest. The independent
Python model computes the same characters with integer arithmetic and checks
the getter output. Neither path executes reference SQL or active database
content.

| Profile | Native bytes | Stored CAB bytes | MSZIP CAB bytes |
| --- | ---: | ---: | ---: |
| V2016 | 77,041,664 | 77,060,558 | 2,798,106 |
| V2019 | 77,111,296 | 77,130,206 | 2,801,250 |

The six preserved files occupy 313,943,080 bytes, below the 500 MiB corpus ceiling.
Both stored packages exceed the former 64 MiB package limit. Both MSZIP packages
are below that limit while their expanded images exceed it. This distinguishes
physical-package and expanded-image ceilings.

The original CAB writer extends the geometry used by the repository's original
`tests/support/deployment.rs` recipe. It writes one folder, one inert
`application.accdb` entry, optional-zero checksums and 32 KiB data blocks. Stored
blocks are copied directly; MSZIP blocks use `CK` followed by an independent raw
DEFLATE stream. It patches only the new CAB header's final length after writing.
Construction and verification never load an entire native image or cabinet into
memory. Blockwise decoding of each package reproduces the exact source size and
SHA-256. The writer is not a deployment signing or preservation implementation.

## Model observations

Both reopened originals agree with the independent model:

- Row count: 1,100.
- Sum of IDs: 605,550; sum of scores: 10,284,450.
- Memo length: 36,044,800 UTF-16 units across all rows.
- Concatenated Memo UTF-8 SHA-256:
  `e8bf0a69f1c219b0eee43da5241c075df4e15e9de0b97c849b89d6581e343425`.

Selected rows 1, 550 and 1,100 also have independent exact payload digests. The
private model report records those observations, the first 64 native header
bytes, package geometry, all six original byte hashes and tool hashes before and
after inspection. The originals remain unchanged.

The fixture-only command is:

```sh
python3 -B docs/testing/compare_large_deployment.py \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --report .cache/large-deployment-original-model-20261025.json
```

The generator requires fresh output paths. Its two modes are:

```sh
java -Xmx256m -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/LargeDeploymentFixture.java NEW.accdb V2016
java -Xmx256m -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/LargeDeploymentFixture.java --inspect NEW.accdb
python3 -B docs/testing/build_large_deployment.py \
  --input NEW.accdb --output-dir FRESH_DIRECTORY --report FRESH_REPORT.json
```

The original fixture inventory found no qualifying reusable file larger than
60 MiB outside `.cache/tablecraft-fixtures`, excluding build artifacts, Git data
and frozen executables. No existing original or accepted report was overwritten.

## Before-change reproduction

The frozen extrema CLI at revision
`ef6d7f50acf9d6068c7ea14bc26f8270a2d8c446`, executable SHA-256
`b46bd18f59033360e5fcdb4e6b5e562e78698353016a8f9ea3a7c61e7e87aeee`,
passes four projected native controls: Count/Min/Max and a complete Id/Score
stream on each raw original. The same projected aggregate query on each stored
and MSZIP package refuses with `database inspection structural limit exceeded`
and empty stdout, despite an explicit 256 MiB native allowance. These eight
controls isolate the old container/image envelope from native table readability.

The ignored baseline is
`.cache/large-deployment-before-spooling-20261025.json`; its terminal log has the
same stem with `.log`. Input hashes agree before and after. An earlier diagnostic
probe expected the word `structure` instead of the actual word `structural`;
its separate ignored log is retained. This was a diagnostic assertion correction,
not a product failure or change to expected query outcomes.

## Frozen CLI acceptance

The [accepted report](results/large-deployment-2026-10-25.json) records 58 controls
across the two raw originals and four packages. The
[runner](compare_large_deployment.py) checks header profiles, catalog row counts,
exact selected Memo payloads through bounded row pages, progressive aggregate
counts and complete projected streams with batch sizes 1 and 128. All controls
agree with the independent models. Six low-byte refusals and four unchanged
default-package-budget refusals retain empty stdout.
Explicit byte allowances remain cumulative; low-budget and default package-budget
refusals are separate controls. Successful row payloads are decoded from the TSV
value representation before exact length and SHA-256 comparison.

Run it with `--cli FROZEN_PATH --cli-sha256 VERIFIED_DIGEST`,
`--revision COMMITTED_REVISION`, `--provenance ORIGINAL_PROVENANCE_JSON` and a
fresh report path after the new build is frozen. Fourteen explicit owning Rust
paths are hashed from raw `git show` snapshots of that commit, before and after
execution; these are not a compilation manifest or live-source pins. The fixture-only report has zero CLI controls
and must not be counted as spooled-path acceptance. The source image digest,
reopened getter values and selected payload models remain independent of that
new executable. No timing or arbitrary-size support claim is made.


The frozen executable is `.cache/tablecraft-bin/spooled-hiragana-cli-20261025`,
SHA-256 `fbf269865c214d09ce3d6cb07dd02eb558860eef142bd72bd008118db57d026b`,
built from revision `bfb5af50a8778185ba24ea99fffa85607e475b3f`.
The original provenance argument is
`.cache/large-deployment-provenance-20261025.json`. All twelve immutable
executable/tool/provenance/original hashes agree before and after replay, as do
the fourteen selected committed source snapshots. The accepted report SHA-256 is
`be6c3033e1069b3fbafe82fc369f0fe3a5e89175b77a68cadbf1b4e352e33f21`;
the runner SHA-256 is
`c1711b06eab9fa7c419f5bf9c973a0a728047d247e0584ce8945aa4eeba704aa`.
The terminal log `.cache/large-deployment-cli-replay-20261025.log` has SHA-256
`07b775fc9ed0563b0ed1b273a3a0f2f42ae6cd5c98337e4ec42ee839ce58b0c7`.
This first new-path replay passed without an excluded executable/oracle attempt.
No original, accepted report or frozen executable was modified.
