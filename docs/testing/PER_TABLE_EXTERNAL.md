# Per-table external source evidence

The accepted 2026-10-24 replay passed **184 commands: 156 successful commands and
28 intended refusals**, with **260 modeled result cells**, zero disagreements and
**1,923 unchanged file pins**. The report is
[per-table-external-2026-10-24.json](results/per-table-external-2026-10-24.json).
The production source is commit `cf7a501b7c008f5971c53df968be21ee54b61836`,
verified against the frozen 1,890-file Rust manifest.

This is independent engineering evidence. Jackcess supplies public metadata and
ordinary record getters; it does not execute the reference SQL. No Microsoft
Access SQL execution, native save/reopen qualification or universal provider
compatibility is claimed. Microsoft's
[remote-reference documentation](https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/access/operation-is-not-supported-error)
establishes the database/table syntax and alias example. It does not establish
native precedence between a global `IN` and a per-table selector.

## Original corpus and reference model

[PerTableExternalFixture.java](PerTableExternalFixture.java) is original repository
MIT authoring code using the reviewed Jackcess 5.0.4 public builders and inert
system-query fragment insertion. It creates fresh files for V2016, V2019, V2000
and V2007. Each profile has a root and two explicitly supplied targets: twelve
original files altogether. The V2000 files use `.mdb`; the others use `.accdb`.

The qualified corpus lives under ignored
`.cache/tablecraft-fixtures/per-table-external-20261024-qualified/`.
Its `generation.json` retains exact authoring/getter commands, stdout/stderr,
source, compiled class, JAR, original database and getter hashes. The exact runner
used for that generation is preserved as `generation-runner.py`; the final replay
runner includes later replay-only guards and diagnostic-label corrections.

Every database disables expression evaluation and installs a link resolver that
throws if asked to resolve an external token. After writing and closing, the
recipe reopens each original read-only and records public query SQL,
`getFromTables`, global remote properties, raw `MSysQueries` fields and ordinary
user records. It checks that no linked database handles were activated. Original
bytes are checked after getter reads and again across the accepted replay.

The root's `Remote` table contains ID 999. The first target contains IDs 1, 2 and
3; the second contains 2, 3 and 4. Root `Local` records supply factors 10 and 30
for IDs 1 and 3. Expected retrieval and join rows are computed from those ordinary
getter records. A target-local saved `TargetView` computes ID times ten. Its
execution distinguishes target lexical ownership from the root's same-named
`Remote` table.

The accepted controls cover direct, saved and named per-table references, aliases,
wildcards, mixed local/remote joins, exact opaque punctuation and Windows-style
selectors, root/target name collisions, target saved-query ownership, missing and
case-changed bindings, an empty-selector refusal, positive and cumulative quotas,
and a late division error with no buffered stdout. The two-target global `IN`
control uses TableCraft's documented engineering rule: an explicit leaf selector
overrides the FROM default namespace. Raw getters confirm both qualifiers are
retained; they do not supply a native execution oracle for that rule.

## Metadata rendering and owned SQL roundtrips

Public Jackcess SQL/from-table getters are metadata observations. In particular,
the punctuated token includes brackets, a quote and SQL-looking punctuation.
Jackcess's rendered nested brackets are not treated as a valid SQL example and
are never reparsed or executed as the reference query.

The independent direct-query recipe instead quotes the exact opaque selector,
doubling each closing bracket. TableCraft's parser prints its owned normalized
model; reparsing that output must produce the identical normalized display.
Separately, native saved reconstruction prints the owned model and the parser
must accept that output. Those saved outputs are also explicitly executed through
host-supplied bindings. No getter-rendered SQL is used to bypass native fragment
validation or to authorize a filesystem/provider open.

## Frozen artifacts

| Artifact | SHA256 |
| --- | --- |
| Accepted report | `5608bec2895ddae5a8f103b1943f89b6fbd2c101096dbb8a947cc98ff1cb8cac` |
| Frozen CLI `.cache/tablecraft-bin/per-table-remote-cli-20261024` | `1e53322d33ee226f48f11d6b712889fb92ce14a089a83e78709eb37c5a944aaa` |
| Rust manifest `.cache/per-table-remote-rust-source-20261024.manifest` | `18f05c27b12f55deed0be398e503b463d1da7df083d2e65a8aeb7c3af1302ec9` |
| Qualified `generation.json` | `3739dd812852a407449c7a5176561bd8cd06c2eb70a2583ed5e4380b1c786a59` |
| Original Java recipe | `3bafee3acf478a1fee8c2ec53b14cc196a511e8d0792824c617b28175ebd9eb0` |
| Compiled recipe class | `468b9f662a8e9c821cdf8d1a244ba73404f90df1673212a655d7d8951ec59ce3` |
| Generation-time runner snapshot | `774d70b0500e2eab3fca34e1d62a201225044ddd6efaf99ee93d63b1198c340c` |
| Final replay runner | `be8f2c053e03fcacb098387983418f27c31c91af0333e8a38e194b137e7c56da` |
| Reviewed Jackcess 5.0.4 JAR | `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc` |

The report contains the individual original/getter/class/source/tool hashes and
all command results. Paths are repository-relative; the Windows-style selector
is intentional input data. The final raw log is ignored
`.cache/per-table-external-replay-final-20261024.log`.

## Reproduction

The generator refuses to overwrite a corpus. To regenerate, choose a fresh
ignored directory; newly authored database bytes receive their own provenance
hashes rather than replacing the accepted originals:

```sh
python3 docs/testing/compare_per_table_external.py --generate \
  --directory .cache/tablecraft-fixtures/per-table-external-reproduced \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar
```

The following reruns the accepted controls against the retained qualified corpus
and frozen artifacts, writing a fresh ignored report. Report overwrites and paths
outside the checkout are refused. The runner checks all pins before and after
execution and retains stdout, stderr and modeled/refusal expectations:

```sh
python3 docs/testing/compare_per_table_external.py \
  --directory .cache/tablecraft-fixtures/per-table-external-20261024-qualified \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli .cache/tablecraft-bin/per-table-remote-cli-20261024 \
  --cli-sha256 1e53322d33ee226f48f11d6b712889fb92ce14a089a83e78709eb37c5a944aaa \
  --source-manifest .cache/per-table-remote-rust-source-20261024.manifest \
  --manifest-sha256 18f05c27b12f55deed0be398e503b463d1da7df083d2e65a8aeb7c3af1302ec9 \
  --report .cache/per-table-external-reproduced-report.json
```

## Preserved excluded attempts

The initial authoring attempt used NULL instead of the qualified flag 2 on the
attribute-4 global `IN` row in `GlobalAndLeaf`. Across four profiles, its eight
reconstruction/execution refusals were correct native-profile enforcement.
Production validation and expectations were not relaxed. The corrected Java
recipe generated twelve fresh files in the qualified directory; the initial
twelve originals remain untouched.

That failed attempt recorded **180 commands and 244 modeled cells**, rather than
the accepted 184/260. Four owned-parser reparse commands were conditional on
successful `GlobalAndLeaf` reconstruction and therefore did not run. The failed
saved executions contributed no successful modeled cells: their four two-row,
two-column results account for the missing sixteen cells. These are consequences
of the authoring failure, not four newly added controls or weaker expectations.

The failed report and exact initial Java, Python and compiled-class snapshots are
retained under ignored `.cache/per-table-external-exploration-20261024/` and were
checked against its original report pins. Its `EXCLUDED.md` records the cause.

A subsequent qualified run passed 184/260 but four diagnostic labels included
host-specific `Path` representations. Its report and runner remain ignored as
`.cache/per-table-external-label-paths-20261024.json` and
`.cache/per-table-external-label-paths-runner-20261024.py`. Only labels changed
before the final accepted replay; query commands, expectations, production source,
frozen CLI and qualified originals were preserved. Neither excluded report is
counted in the accepted totals.
