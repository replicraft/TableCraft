# Single Int32 ignore-NULL index evidence

The [64-command comparison](results/ignored-null-index-replay-2026-10-25.json)
passes against four original public-writer inputs. It compares 3,468 returned IDs
with read-only getter values and index-cursor membership. There are 56 current
executable controls and eight baseline controls: 52 successful commands and twelve
expected selected-index refusals overall. This is independent engineering evidence,
not native Microsoft Access execution or compatibility qualification.

## Original inputs and oracle

Original Apache-2.0 [IgnoreNullIndexFixture.java](IgnoreNullIndexFixture.java)
uses the reviewed Jackcess 5.0.4 public builders to create 701 ordinary rows per
V2000/V2007/V2016/V2019 input. Explicit expression evaluation is disabled and linked
resolvers refuse every open. No third-party source, fixture or active content is
copied or executed. The producer uses fresh paths and never overwrites an input.
Originals remain ignored under
`.cache/tablecraft-fixtures/ignore-null-indexes-20261025/`.

Each input has ascending/descending ignored Int32 keys, a logical alias sharing
physical data and a unique ignored Int32 key. Ordinary keys retain 637 non-NULL
entries and omit 64 roots; the unique key retains 647 and omits 54. Every physical
index owns three pages. Public row/index-cursor getters supply exact integer/NULL
values and encounter order. Pinned reference implementation metadata getters
supply physical data numbers, counters, owned-page counts and table geometry;
the oracle does not claim to use exclusively public metadata APIs. Baseline inert
index inspection locates the roots; independent byte checks confirm sixteen
logical root references identify node pages of kind/version 3/1 with the expected
table owner. The ascending alias shares physical index zero.

The [runner](compare_ignored_null_indexes.py) independently filters the actual
getter rows. Required-index expectations use captured cursor order; automatic
expectations use captured sequential encounter order. It also verifies that cursor
membership is exactly the non-NULL getter population and follows integer key order.
Only TableCraft executes the original SQL controls; no reference SQL is executed.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| Ignore-V2000.accdb | 159,744 | `6f608a93375a3ef88941b4e5c7f7bb2922709a9ba2272e6a8750002ae3fd0114` |
| Ignore-V2007.accdb | 389,120 | `d3add63c222c3ce805bbac8188ba8791c65c8e4fcba022cadb8d53fd5fca9875` |
| Ignore-V2016.accdb | 425,984 | `1b7afd89440039b877ae55e700dd07c0bb0e0ed7fab6a5909aaf9073f3489a16` |
| Ignore-V2019.accdb | 495,616 | `6246f75593d6ca7a7b9f71f7009cbf1627d8b50eef9504f6df14fba8b9122898` |

## Observed behavior and boundaries

Current full checks validate all four logical indexes and retain the full 701-row
count. Baseline checks report all four as unsupported. Explicit equality and
bounded ranges match ascending/descending/unique/alias cursor membership while
projecting only ID, so the selected reader still validates unprojected keys.
Automatic equality/ranges preserve sequential order; automatic NULL tests decline
the omitting index and return the actual NULL roots. Required All/NULL reads still
refuse, and the baseline also refuses the newly admitted non-NULL equality read.

The [implementation verification](results/read-query-ignored-null-int32-2026-10-25.json)
records 65 unique focused Rust tests, eleven new, plus strict engine/adapter lint
and the adapter minimal-feature check. These include corruption, missing/extra/
NULL entries, unique/required violations, empty populations, cancellation and
resource exhaustion. Existing normal Jet 4 and ACE16 engineering-preview mutation
refusals remain explicit, including empty plans and unrelated-column updates.
Other ignored carrier/composite profiles are not qualified by this evidence.

The first strict lint rejected an eight-argument private helper. Its projection
and eligible population were bundled into a narrow request, then the complete
focused engine/adapter tests and lint were rerun. The earlier passing test logs
and initial lint failure remain excluded from the final unique-test count. The
original CLI comparison itself had no failed or corrected run.

## Artifact identity and reproduction

Current frozen CLI:
`.cache/tablecraft-bin/ignored-null-int32-cli-20261025`, SHA-256
`1128c3a6e9b8a29bf667fea81d1c94809b6d7abe9566c9826e0ebd7f7bb24db7`.
Baseline frozen CLI:
`.cache/tablecraft-bin/inspect-native-work-cli-20261025`, SHA-256
`1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0`.
Seventeen original/tool/getter/metadata artifacts are unchanged before and after.
Six selected native source objects match revision `9cb3e387` and their live files
before/after. The executable was built with unrelated uncommitted query/format
work present: its immutable hash identifies the tested artifact, rather than a
claim that the entire binary came from a clean checkout of that revision.

The raw command/output log is
`.cache/ignored-null-index-replay-20261025.jsonl`; terminal session 11016 exited
zero. The report records its hash, every exact command and all observed outputs.
The producer/getter source is a reproducible recipe, not a promise of identical
file bytes across new runs, timestamps or reference versions.

```sh
# Verify the reviewed JAR before using the original recipe:
# SHA256 19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/IgnoreNullIndexFixture.java generate FRESH_PATH V2016
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  docs/testing/IgnoreNullIndexFixture.java oracle FRESH_PATH

python3 docs/testing/compare_ignored_null_indexes.py \
  --cli .cache/tablecraft-bin/ignored-null-int32-cli-20261025 \
  --cli-sha256 1128c3a6e9b8a29bf667fea81d1c94809b6d7abe9566c9826e0ebd7f7bb24db7 \
  --revision 9cb3e387 \
  --report FRESH_REPORT --log FRESH_LOG
```

Use the same recipe for each of the four explicit profiles; no imported formula,
provider, network access or native write API is enabled by this read qualification.
