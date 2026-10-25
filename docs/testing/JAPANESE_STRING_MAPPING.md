# Japanese StrConv primary mapping evidence

The 25 October 2026 replay compares TableCraft's explicit Japanese string codec
with an independently authored integer/index model of the published
[WHATWG Shift_JIS algorithms](https://encoding.spec.whatwg.org/#shift_jis-decoder).
It passes **2,172 CLI commands**, checking **17,314 mapping cases** and
**32,158 numeric cells**, with zero disagreements. The
[accepted report](results/strconv-japanese-primary-2026-10-25.json) retains every
command, expected cell, stdout/stderr, immutable input hash and scoped compiled
source hash. [The feature check record](results/read-query-japanese-string-encoding-2026-10-25.json)
keeps this replay separate from its 47 scoped Rust tests.

This is codec and strict round-trip engineering evidence. It does not establish
Microsoft Access execution, Windows NLS behavior, native default code pages,
best-fit conversion or performance. The implementation contract remains in
[JAPANESE_STRING_ENCODING.md](../JAPANESE_STRING_ENCODING.md).

## Original tools and private reference data

[compare_strconv_japanese.py](compare_strconv_japanese.py) and
[strconv_japanese_model.py](strconv_japanese_model.py) are original repository
MIT tools. The model reads primary reference files as data and applies their
published pointer arithmetic. It imports neither the production codec nor a
Python encoding implementation as its oracle. No reference mapping table,
third-party fixture or upstream implementation is copied into the repository.

| Private input | Primary identity | SHA-256 |
| --- | --- | --- |
| `.cache/cp932-strconv-next-20261025/CP932.TXT` | [Microsoft-authored mapping](https://unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP932.TXT), table 2.01, Unicode 2.0, 1998-04-15 | `c9bc0b0cd42e0fbcb82a09635bb5abed86afbdd4abc9e76fa5716638217cb59f` |
| `.cache/cp932-strconv-next-20261025/index-jis0208.txt` | [WHATWG JIS0208 index](https://encoding.spec.whatwg.org/index-jis0208.txt), 2024-09-18 | `341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468` |

The WHATWG index identifier is
`cbaa91f3deb7d0841faf5c33041fc15a285da0e87e64ab802c4bf04b7c4da861`.
The [Unicode data license](https://www.unicode.org/license.txt) and
[WHATWG licensing terms](https://encoding.spec.whatwg.org/#ipr) were reviewed.
The downloaded files stay ignored and private; their hashes establish exact
inputs rather than permission to redistribute them. The runtime consumes the
existing reviewed `encoding_rs` dependency, separately from this model.

The frozen executable is
`.cache/tablecraft-bin/japanese-string-cli-20261025`, SHA-256
`398b9c4f4b9e41521c2fd70750b5f946d5fcfa38133f3f66304f11c92ea91808`,
built from revision `1286af1ea0ea39c0d350c4d64bb235fb8132f196`.
The report pins thirteen source files through raw `git show REVISION:PATH`
bytes. It does not require an unchanged live working tree or a mutable full-tree
inventory. Five immutable pins—the executable, both reference files and both
original Python tools—match before and after execution. All five and all thirteen
compiled-revision source pins were independently rehashed after the replay.

The accepted report SHA-256 is
`dcf75f2fda2db87219de179df086a82fee1102e1955918918262ac24b1c9a163`.
The runner SHA-256 is
`0e8e3f5c0ef733e35b52e2c0f98926027faa496f70ebf22f71dc18eb90d5f614`;
the independent model SHA-256 is
`a22bee9c7a7503014f161432c0a85a54aadc81ccbf4f57a0f06800ee035ec21e`.

## Matrix and representation boundaries

The successful matrix covers every defined one-byte and lead/trail token in the
chosen decoder profile, and every canonical encoding selected from the supplied
index plus its single-byte ranges. It uses 2,102 successful batches with at most
16 projected columns. Seventy separate commands test intended refusals.

| Control | Cases | Expected behavior |
| --- | ---: | --- |
| Defined decoder tokens | 9,796 | Exact Unicode scalar, observed through signed `AscW` output |
| Canonical encoder mappings | 7,518 | Exact `LenB` and each encoded byte through `AscB` |
| Undefined or incomplete singleton tokens | 64 | Nonzero status and no buffered stdout |
| Selected refused Unicode inputs | 6 | Nonzero status and no buffered stdout |

Exact two-byte decoder inputs are constructed as one packed UTF-16 unit using
`ChrW(low + 256 * high)`, including units that are isolated surrogates. Conversion
64 consumes the retained bytes directly. Singleton tokens use `ChrB` directly.
The runner does not concatenate odd-byte String operands: that operation has its
own established refusal before codec admission. Encoder inputs use `ChrW` or a
valid supplementary literal, so embedded NUL never enters a subprocess argument.

All 7,915 defined Microsoft 1998 byte mappings agree with the chosen WHATWG
decoder. The additional 1,881 decoder mappings are classified separately: byte
`80` produces U+0080, and 1,880 EUDC pairs from `F0 40` through `F9 FC` produce
U+E000 through U+E757. Those extensions are not attributed to the historical table.

Canonical encoding follows the WHATWG index exclusion/first-pointer rules.
There are 398 defined alternate byte tokens across 396 Unicode scalars whose
canonical encoding differs from that alternate spelling. Both decoding and
canonical encoding are checked; arbitrary byte round trips are not promised.
For example, both `EE EF` and `FA 40` decode to U+2170, while encoding selects
`FA 40`. Python's preferred aliases are not the expected answers.

The 64 refused singleton tokens comprise 60 dangling lead bytes and `A0`, `FD`,
`FE`, `FF`. The six refused Unicode inputs are U+00A5, U+203E and U+2212, which
would lose Unicode identity through best-fit aliases; the decoder-only EUDC
endpoints U+E000 and U+E757; and the unmappable supplementary scalar U+1F600.
This finite refusal set does not claim exhaustive testing of every invalid
multi-byte sequence or every unrepresentable Unicode scalar. Focused Rust tests
separately cover invalid trails, active ranges, raw/Binary operands, NULL/eager
controls and resource/cancellation boundaries.

## Reproduction and preserved first attempt

The accepted command was:

```sh
python3 docs/testing/compare_strconv_japanese.py \
  --cli .cache/tablecraft-bin/japanese-string-cli-20261025 \
  --cli-sha256 398b9c4f4b9e41521c2fd70750b5f946d5fcfa38133f3f66304f11c92ea91808 \
  --index .cache/cp932-strconv-next-20261025/index-jis0208.txt \
  --microsoft .cache/cp932-strconv-next-20261025/CP932.TXT \
  --revision 1286af1ea0ea39c0d350c4d64bb235fb8132f196 \
  --report docs/testing/results/strconv-japanese-primary-2026-10-25.json
```

The runner refuses to overwrite a report; choose a fresh report path for a new
execution. It has subprocess timeouts, retains matched refusal stderr, and checks
all immutable pins before and after. The accepted raw log remains at
`.cache/strconv-japanese-primary-replay-final-20261025.log`.

The first attempt used batches of up to 64 projected columns. Its 578 commands
comprised 508 intended success batches and 70 intended refusals. All 70 refusal
controls matched, but every success batch exceeded an existing CLI evaluation
ceiling: 153 exhausted expression work and 355 exceeded SQL token limits. It
therefore recorded **508 disagreements**, not a successful mapping comparison.

The exact failed report, runner, model and raw log remain unchanged under
`.cache/cp932-primary-batch-exploration-20261025/`. Its report SHA-256 is
`aa5629178e1ee8be985b70c0b8b2c376a2cf8e9ef553d3da8d1d1bef52f83c70`.
Reducing batching to 16 columns produced the accepted 2,172-command run while
preserving all 17,314 mapping cases and all 32,158 expected numeric cells. No
runtime, reference table, original fixture or mapping expectation changed.
