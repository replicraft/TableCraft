# Decimal secondary INSERT evidence

This repository-authored corpus covers 48 cases on 48 generated V2016/V2019
originals. Each table has manual required Int32 ID and nullable Decimal Amount;
Peer is nullable GUID or a separately declared Decimal. Four logical index names
share exactly two physical owners, with a required unique ascending ID primary.

The observed cases are:

- Sixteen fitted single Decimal cases: precision/scale (1,0), (3,2), (28,4) and
  (28,28), ASC/DESC under both profiles.
- Four single Decimal pressure cases at (28,4), ASC/DESC under both profiles.
- Sixteen Decimal/GUID pressure cases, both written orders and all four
  component-direction combinations under both profiles.
- Twelve fitted two-Decimal cases: Amount(28,4) and Peer(3,2), both written
  orders with directions AD, ordinary/unique/unique-ignored under both profiles.

Actual raw metadata binds each field's own precision, scale, flags, fixed
spans and variable ordinal. BigDecimal getters are checked by exact coefficient
and scale; no floating conversion, rescaling or neighboring-field scale is
borrowed. A present Decimal occupies 17 variable bytes: a sign byte followed by
most-significant-first 32-bit words, each word little-endian. NULL has an absent
presence bit and is distinct from present Decimal zero. Source NULL payloads
and padding remain opaque and preserved. Partial-NULL tuples stay eligible;
IgnoreNULL excludes only all-selected-NULL tuples. Historical secondary counters
track new distinct eligible key families, separately from inserted row counts.

The first public-writer experiment completed 289 calls: one compilation, 48
source generations and 96 source reads, followed only after all 48 source gates
passed by 48 disposable-copy appends and 96 reference reads. Raw records,
getter carriers, reflected OLD/NEW createEntryBytes outputs, full key/ROW streams,
usage maps and pressure geometry are separate checked observations. Twelve known
omitted-NULL warning events use the existing exact warning grammar; all other
stderr must be empty. No source or reference was repaired or regenerated.

All 48 engineering images were derived solely from qualified SOURCE bytes and
exact requests and frozen before the isolated committed-source producer build.
The first 48 producer calls matched the complete expected images before any of
the first 96 fresh getter/map calls ran. The resulting cases preserve 2,252 old
logical ROWs, insert 160 rows and contain 2,412 final rows and 4,812 physical
key/ROW pairs. They allocate 40 fresh INDEX pages and no new DATA pages.
Public-writer framing and scratch bytes never construct candidate goldens.

Run the saved replay from the repository root:

```sh
python3 -B docs/testing/ace16_insert_decimal_indexes/verify.py
python3 -B docs/testing/ace16_insert_decimal_indexes/verify.py --verify-git
```

The default starts no child process. The explicit Git option starts one metadata
batch binding the complete archived compiled source and committed build receipt.
Neither mode invokes Java, a producer or a fixture generator. Missing ignored
evidence returns `available: false` and exit status 2 without generating replacements.
Use `--evidence-root PATH` for a complete relocated evidence tree.

`recipes.py` and the Java helper retain exact private bytes. `reference.py` and
`stderr_policy.py` change only repository bootstrap and owning-package imports;
the verifier checks exact transformations and all function ASTs. `model.py` uses
the actual maintained multiple-index framing owner; its constructor functions
remain exact, with only the repository root adjusted. The independent model
has its own Decimal key and new-record encoders, and never imports the source
proof's encoders or consumes public AFTER/Java NEW bytes as construction authority.

The saved verifier rechecks all source gates before deriving any complete image,
independently decodes each new record and key stream against exact requests,
replays public transitions after derivation, compares all candidate bytes before
interpreting cached fresh getters, and rehashes all observed pins at completion.
The original getter static summary printed 32/64 while its actual inventory and
caps already checked 48/96. Exact initial files and output are archived; only
those display literals were corrected before candidate/getter execution. This
pure authoring history is separate from the successful first actual phases.

Original databases, JARs, classes, raw streams and expected images remain ignored
private evidence. No third-party fixture or implementation source is copied.
See the [compact result](../results/ace16-insert-decimal-index-native-2026-10-28.json)
and [fixture provenance](../FIXTURES.md).

These observations qualify the finite engineering and independent-reader cases
above. They do not establish arbitrary Decimal tuples, native Microsoft Access
saves/reopens, data macros, Windows integration or normal production write paths.
