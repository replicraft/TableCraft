# Test duplication audit — 1 November 2026

The suite contains genuine copied support code, but its size is not evidence of
400,000 lines of duplicate coverage. This pass consolidates clear copies while
retaining all named tests, fixture profiles, independent byte expectations and
layer boundaries. It does not claim that automated matching proves semantic uniqueness.

## Inventory and method

The baseline is commit `f3697466`. A static inventory of tracked Rust files under
`crates/` and `apps/`, following test-gated modules and including integration tests
and the engineering testkit, identifies 488,010 physical test/support lines in
2,638 files. Physical lines include comments, blank lines and fixture data. There
are 7,447 literal `#[test]` attributes and 555 `#[ignore]` attributes across active
Rust sources; these are source occurrences, not a count of unique, expanded or executed cases.
Twelve tracked Python test files add 1,307 physical lines outside that inventory.

| Area | Physical test/support lines |
| --- | ---: |
| Native ACE engine | 231,035 |
| Query binding/execution | 83,780 |
| Access adapter | 50,309 |
| CLI | 42,223 |
| Expressions | 37,960 |
| Independent fixture testkit | 29,059 |
| Other Rust packages | 13,644 |

The structural scan compared whole-file hashes, normalized function bodies of at
least twelve physical lines, and matching blocks of at least twenty substantive
lines. Comments and indentation were excluded from block matching. Manual review
separated shared machinery from independent oracles and different contracts.
Archived evidence under `docs/testing/` and ignored private scratch material were
excluded from active totals and cleanup.

There were no byte-identical whole active test/support files. Function matching
flagged 120 candidate groups. Block matching flagged 460 pairs involving 30,542
physical lines across **both sides** of matches, after merging overlapping ranges.
Those figures are review candidates, not redundant tests or safely removable
lines. Function names alone were disregarded; different fixture constants,
scopes, source populations and traits can make otherwise identical bodies test
different behavior. Shorter or substantially rewritten copies can evade this scan.

## Consolidated support

Shared machinery now belongs to `tablecraft-testkit`, organized under
`fixtures/{access,formats}` and `support/{ace,query,access,cli}`. Tests import the actual
owner directly; former local helper paths do not forward or reexport it. Default raw fixtures use Core carriers and the existing UUID byte-order utility;
compressed CAB construction is optional. Test consumers opt into the subsystem
support features described in the [testkit guide](TESTKIT.md).

- Five identical Sentinel inspection helpers share
  `support::ace::sentinel`. Each caller supplies its original catalog scope and
  budget factory. Literal constructors, manual byte reconstruction, hash pins and
  all thirty source views remain separate.
- Two Access-session scenarios and their two CLI counterparts use
  `support::access::owned_growth` and `support::cli::owned_growth`. Both
  storage-placement families, both headers, typed/SQL paths, retained leases,
  exact images, receipts, namespace cleanup and CLI readbacks remain. The CLI
  tests pass their executable path explicitly; the testkit has no application
  dependency.
- Four multivalue query sources share `support::query::multivalue` Source, DTO and
  assertion machinery. Their different list types, nullability, fixture
  populations, expected rows, cursor guards and fault hooks remain explicit.
  Explicit text-policy metadata behavior remains distinct from the ordinary
  source default.

The initial query extraction reduced four support files from 1,451 to 490 lines
(961 lines), and the Sentinel extraction removed 456 copied lines. Host workflow
extraction removed 86 lines before relocation; its testkit move added one line
before module composition. These measurements have different scopes and must not
be added into a final suite reduction.

The shared headless CLI runner is configured once per integration target rather
than repeating the executable at every call. Static inverse comparisons preserve
474 test attributes across 134 runner configurations and 855 calls. Independent
The broader CLI run exposed three stale expectations. Help now checks the
implemented journal update/recovery capability and the retained GUI limitation.
The UTF-16 addition and concatenation tests now require two paired byte fragments
to produce a two-byte result, matching the existing
[String join contract](../../../docs/expressions/text/CONCATENATION.md#odd-byte-join-verification),
while retaining invalid-Unicode publication and eager-error refusals. These are
test corrections; production behavior is unchanged.
CAB construction also moved to `fixtures::formats::deployment`. These ownership
moves are useful even where explicit imports and formatting add physical lines;
line reduction alone is not their acceptance criterion.

A later static inventory records 486,697 physical test/support lines in 2,644
files, with the same 7,447 test and 555 ignore source occurrences. It flags 105
function groups and 446 block pairs. This is a recorded cleanup snapshot, not a
new compatibility result or proof that the remaining cases are semantically
unique. The final verification below completed on 2 November 2026.

## Redundant executions

Dense numeric and wider-primary tests previously repeated empty INSERT and
DELETE admission against a fresh copy of the same original for each different
AFTER recipe. The block used neither that recipe nor its AFTER image. Running
once per `(table, full original digest)` retains twelve dense and four
wider-primary source contexts while reducing these empty plans from 88 to 32.
All 44 principal mutation cases remain.

The same unsupported comma-join/DISTINCT SQL row also appeared in both INNER and
LEFT collection suites with identical sources, execution and assertions. It now
runs in the INNER suite once. Both actual LEFT-specific refusal cases remain;
this does not remove an outer-join boundary or a named test.

## Repetition retained deliberately

Independent raw record, index-key, wire and whole-image builders must remain
separate from production encoders. Making both sides call the same encoder could
hide precisely the corruption these tests are intended to detect. Source hash
pins repeated at read-qualification and mutation boundaries similarly bind their
respective evidence; shared pins alone would not replace independent byte replay.

Engine planning, fresh proof, journal recovery, retained adapter sessions and CLI
publication test different contracts even when they use the same database.
Partial writes, committed-marker mismatches, cancellation, resource exhaustion,
lease retention, exact output and stale handles must not be collapsed into a
single successful save. INNER/outer joins, NULL/absent collections, different key
carriers, compression, storage layouts and sequential edits also remain distinct
cases.

Smaller callback helpers, resource/recovery harnesses and some source inspectors
remain possible future extractions. They should be parameterized when the same
observations recur, without replacing independent expected bytes or creating a
generic framework solely to save a few local lines. This audit found copied
plumbing worth removing; it did not establish that whole feature suites can be
deleted as equivalent coverage.

Verification and recorded cleanup measurements are in the
[audit record](../../results/general/test-duplication-audit-2026-11-01.json).

## Verification status

Verification completed on 2 November 2026. All 211 CLI integration targets have
passing results across the stopped broad run and focused continuations after the
three stale expectations were corrected. The stopped failures remain in the
record; targets requiring private fixtures retain their normal ignored status.

The ACE unit batch passed 1,171 functions with 218 ignored. All forty multivalue
query tests, eight moved-fixture integration targets and both deployment targets
passed. Five explicitly selected Sentinel checks covered thirty source views;
both retained Access workflows, both CLI growth workflows and both empty-plan
deduplication checks passed with their available private fixtures.

Workspace all-target/all-feature compilation, strict Clippy for the five affected
packages, focused strict Clippy for the three corrected CLI targets, formatting,
minimal ACE/Access and default/all-feature testkit checks passed. All seventeen
dependency-guard tests and the boundary guard passed. Static inverse comparisons
and dependency-tree checks supplement these runtime results. None of this is a
native Access or Windows compatibility qualification.
