# Compressed-text wide-segment grammar

The native text decoder previously refused a wide UTF-16 code unit when its high
byte was zero. That imposed a canonical compression choice absent from the
reviewed decoder grammar. A zero switches modes only at the next character
boundary; a zero consumed as the high byte of the current wide unit is data.
For example, original bytes `FF FE 00 41 00` decode to `A`.

The correction removes that high-byte restriction. Marker recognition, character
boundary switches, pair completeness, strict UTF-16, decoded/encoded limits,
fallible allocations and cancellation remain unchanged. The shared decoder
serves native names, ordinary ShortText, Memo and existing qualified wrapped text
consumers; it does not admit new native flags or formula functionality.

## Independent grammar evidence

The owning reading reference is pinned
[MDB Tools iconv.c](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/iconv.c).
Its `decompress_unicode` checks zero at the current character boundary and copies
both bytes of a wide unit without testing the high byte. Jackcess 5.0.4
`ColumnImpl.decodeTextValue` independently scans by one byte in narrow mode and
two bytes in wide mode; its public `ColumnImpl.read(byte[])` decodes the original
streams in this corpus. The review uses those algorithms as grammar evidence,
without copying third-party source or treating permissive replacement of invalid
UTF-16 by another implementation as a valid stored Unicode contract.

The original [recipe/oracle](CompressedWideFixture.java) creates ordinary seed
ShortText/Memo tables with public builders and ASCII values. The
[runner](compare_compressed_wide.py) authors 259 original encoded streams:
every nonzero low byte 1–255 paired with high zero, repeated mode transitions,
zero boundary switches, supplementary Unicode followed by wide ASCII, and a
127-unit wide segment. Independent expected Unicode strings are compared with
public ShortText and inline-Memo decoder calls.

Explicit derived copies replace only the same-width ShortText and Memo payload
ranges, including inline and single-external-row Memo storage, resolved from
independent physical RowId getters and checked
record framing. Original seed bytes, metadata, row framing and descriptors stay
unchanged. A separate read-only public getter replay reads every resulting
stored value. These are authored **grammar controls**, not evidence that the
ordinary reference encoder or native Access itself generates those layouts.

Additional derived copies replace the final wide unit with an unpaired surrogate.
Queries fail without stdout under TOP or a predicate matching an earlier valid
row. Selecting the other intact text column still works, proving projection
pruning without publishing a failed selected suffix. Independent reference text
replacement behavior is not used to relax TableCraft's strict UTF-16 refusal.

The exploratory ordinary-writer test beginning with literal U+FEFF is excluded.
Its uncompressed bytes collide with the compressed marker, and the independent
writer/getter pair itself loses or changes the intended string. That issue neither
qualifies literal-leading-BOM writes nor supplies this correction's expected
decoded strings. No metadata flags, mutation capabilities or write permits change.

## Verification

Three focused decoder tests cover all high-zero units, mode boundaries, odd tails,
unpaired surrogates, encoded/decoded and allocation ceilings, and cancellation
within a larger wide segment. The existing strict compressed-text test passes
with its former high-zero rejection intentionally removed. Two integration tests
cover ordinary ShortText and inline Memo under the existing ACE 2007/2016/2019
read profiles, including late malformed tails that return no snapshot.
Scoped engine strict Clippy, formatting and Python compilation pass.

The [accepted report](results/compressed-wide-2026-10-24.json) records 542 passing
controls on 24 October 2026: 518 independent ShortText/inline-Memo stream pairs,
two complete stored getter comparisons, six ordinary row/query comparisons,
four resource refusals and twelve late-tail/pruned-column controls. The main
row/query comparisons check 4,662 displayed cells across V2016/V2019 and both
ordinal/database modes. Seventeen source, reference, stream, original seed,
derived copy and executable hashes remain unchanged after replay.

The immutable CLI SHA-256 is
`91d0b78a139b582711fe665d2ecb0e369db906aa5a9b64c60cd5dd36bb2756de`.
A separate supplementary reproduction with the previous immutable CLI
`4f3049ae09612ea9c54ecbf7a6bffff62b1fd2d7e22026619e821609674903eb`
refuses both unchanged grammar copies as malformed, without stdout. Those two
before-fix failures are not included in the accepted control count.

[Fixture provenance](FIXTURES.md#original-compressed-text-wide-segment-controls)
records original seeds and explicitly derived copies. No native Access SQL,
native writer validity, application execution, freshness or save is qualified.

```sh
python3 docs/testing/compare_compressed_wide.py \
  --cli .cache/tablecraft-bin/width-wide-grammar-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/compressed-wide-final-20261024 \
  --report /tmp/compressed-wide-replay.json
```

For a fresh corpus, add `--generate` and choose a new directory. `--derive` creates
fresh grammar/refusal copies from existing seeds; ordinary replay changes neither.
The runner requires the pinned MDB Tools source at the ignored path recorded in
the report and verifies its hash before using it as recorded grammar evidence.
