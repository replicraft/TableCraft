# Existing-slot baseline on encoded split sources

The frozen existing-slot engineering consumer now admits all six
[General Text/Binary source targets](ENCODED_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
without changing their bytes. It refuses all six requested growth operations
with `IndexedColumn` before publishing an output. These are actual native engine
calls on the unchanged originals, separate from source construction or public
writer actions. They do not establish a working split candidate or normal save.

## Consumer and requests

The consumer is the [committed existing-slot example](results/ace16-slot-example-build-2026-10-26.json)
compiled from `59c87310ad4d25fa182336640a85477f5075455c`. Its frozen artifact is
17,017,616 bytes, SHA-256
`ba3b1f5f80c0274d0145006e22ae60a1fc8b7b051576ae651e7a853da36e285b`.
The artifact and all 75 selected historical source objects were independently
verified before these calls. That selected inventory is not an exhaustive
compiler-input closure; the committed revision identifies the source tree.

Each invocation uses the real quoted TSV slot parser, with no flags. Requests
select logical row zero and column `KeyValue`. Text uses UTF-8 `aaa` as its
expected value and either `aaa` or `zzz` as replacement. Binary uses hex `00`
and either `00` or `ff`. No numeric-only parser stands in for storage admission.
The original parser's 1,024-byte value ceiling covers all these inputs.

| Source format | Target | No-op result | Changed result |
| --- | --- | --- | --- |
| V2016 | TextLegacy | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2016 | TextModern | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2016 | BinaryOne | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | TextLegacy | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | TextModern | Byte-identical, zero changed rows | IndexedColumn, no output |
| V2019 | BinaryOne | Byte-identical, zero changed rows | IndexedColumn, no output |

## Actual run and saved evidence

The first execution completed all twelve calls without retry: six no-ops followed
by six changed requests. Successful no-ops returned exactly zero changed rows;
expected refusals returned exit status one and the exact `Unsupported(IndexedColumn)`
error. Every changed output path remained absent. Both source files and all
twenty-five frozen inputs plus the freeze remained unchanged.

The controller enforced a 30-second deadline and a live 16 MiB ceiling per
captured stream, retaining prefixes and partial artifacts on failure. The native
consumer retained its compiled 256 MiB allocation, 100,000-row and 100,000,000-step
allowances. All output names were fresh. No JVM, new compilation, source
regeneration or public UPDATE ran in this phase.

The [compact receipt](results/encoded-index-split-old-slot-native-2026-10-27.json)
pins the plan, controller, raw report and saved proof. The
[later verifier](encoded_index_split_sources/verify_baseline.py) rechecks command
arguments, exact TSV carriers, exit status, captured streams, no-op bytes,
refusal publication and immutable identities without starting a child process:

```sh
PYTHONPATH=docs/testing python3 -B docs/testing/encoded_index_split_sources/verify_baseline.py
```

The earlier failed compile and corrected five-command source phase remain
historical source evidence. These twelve native calls are a separate baseline;
they must not be counted as public writer updates or successful split candidates.
NULL/empty cases, compound or unique overflow, broader collations, native Access
qualification and production saves are outside this result.
