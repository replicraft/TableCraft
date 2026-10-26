# Explicit width-then-Hiragana composition evidence

The original [replay runner](compare_strconv_wide_hiragana.py) checks the explicit
Japanese `StrConv(text,36,1041)` engineering profile. It reuses the independent
Unicode width-property and Unicode-name models in
[the width runner](compare_strconv_width.py) and
[the kana runner](compare_strconv_kana.py), in that order. It imports no production
mapping code and introduces no third-party mapping files. Python's Unicode
16.0.0 database supplies the same reviewed property/name data used by those
standalone models; ten literal goldens independently check their composition.

This establishes agreement with TableCraft's documented composition. It does not
establish native Access StrConv evaluation order, Windows NLS equivalence, or
support for other combined flags. The primary Microsoft references and the
explicit engineering boundary remain in [STRCONV.md](../expressions/text/STRCONV.md#explicit-wide-then-hiragana-composition).

## Accepted replay

The [accepted report](results/strconv-wide-hiragana-2026-10-25.json) records
230 commands: 206 successful batches containing 616 expected cells and
24 intended refusals. All matched, with no excluded attempts. Both `query eval`
and `query run` execute the same controls, with three projections per successful
batch to retain their existing execution ceilings.

The 308 successful controls per command mode cover all 160 qualified width
single mappings, every one of the 122 halfwidth base-plus-voicing combinations,
11 mixed inputs, 12 whole/selected raw inputs, NULL, and two operation-order
discriminators. The halfwidth combinations include both composable and
uncomposable pairs. Mixed inputs distinguish combining and spacing marks,
fullwidth voiced letters, standalone marks, punctuation, preserved compatibility
characters, astral text, tabs, newlines and SQL-composed NUL. Selected raw ranges
exclude invalid UTF-16 prefix/suffix units before strict publication.

The order discriminator explicitly expects mode 36 on `ｶﾞ` to return `が`, while
standalone Hiragana followed by standalone width conversion returns `ガ`.
Refusals preserve missing/wrong/alternate/NULL locale controls, unsupported or
contradictory flag combinations, malformed retained strings, eager source
failures and eager locale failures. Each refused command must fail with empty
stdout; its actual stderr remains in the report. This replay does not claim
native error-priority qualification or exhaustive malformed-input coverage.

## Provenance and reproduction

The frozen executable is
`.cache/tablecraft-bin/spooled-hiragana-cli-20261025`, SHA-256
`fbf269865c214d09ce3d6cb07dd02eb558860eef142bd72bd008118db57d026b`,
compiled from revision `bfb5af50a8778185ba24ea99fffa85607e475b3f`.
The report hashes the frozen executable and four original Python tools before
and after the replay, plus fourteen raw `git show` source blobs from that
compiled revision. All five immutable pins and fourteen revision pins were
independently rehashed after the run. Live checkout source hashes are not used
as a claim about the compiled executable.

The report SHA-256 is
`f3886d3039af95a2b36bbccab689f975a3b750e1c510a51f40c624edfaf668b6`.
Session 95122 completed successfully; the retained local command output is
`.cache/strconv-wide-hiragana-replay-20261025.log`.

Run from the repository root with a fresh report path; the runner refuses
an existing destination and checks the supplied executable hash before running:

```sh
python3 docs/testing/compare_strconv_wide_hiragana.py \
  --cli .cache/tablecraft-bin/spooled-hiragana-cli-20261025 \
  --cli-sha256 fbf269865c214d09ce3d6cb07dd02eb558860eef142bd72bd008118db57d026b \
  --revision bfb5af50a8778185ba24ea99fffa85607e475b3f \
  --report docs/testing/results/strconv-wide-hiragana-2026-10-25.json
```

The bounded runner permits at most 300 commands and applies a 120-second timeout
to each child process. It retains command arguments, expected/actual rows,
exit status, stdout and stderr, including matched refusals and disagreements.
No original databases, native formulas or active document content are involved.
