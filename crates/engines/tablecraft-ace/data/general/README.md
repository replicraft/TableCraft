# General weight data

These are normalized Apache-2.0 Jackcess 5.0.4 weight tables for General legacy
and General 2010, including surrogate-unit weights and the kana vowel mapping.
Read [NOTICE](NOTICE) and [LICENSE-APACHE-2.0.txt](LICENSE-APACHE-2.0.txt). The Rust
encoder in `src/collation/key/encode.rs` and `tail.rs` is also an Apache-2.0
adaptation. Original TableCraft code also uses Apache-2.0; the package inherits
that workspace SPDX identifier. Retain the bundled attribution and notices.

Inputs reviewed:

- `com.healthmarketscience:jackcess:5.0.4` binary JAR, SHA-256
  `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
- Matching source JAR, SHA-256
  `4de1bc2f1663b0946ce39057dd90eace355f4f74b8f818c0fe167f0c65815428`.
- Package POM, upstream Apache-2.0 license and source-file copyright notices.
  Upstream has no additional NOTICE in the reviewed artifact; the retained
  attribution here identifies the data and Rust adaptation.

The engineering exporter calls the reference handler for all 65,536 UTF-16 units
in both families. It records type, character/primary counts, secondary modifier,
placeholder, kana/repeat metadata and primary/secondary/unprintable/tertiary
bytes. It separately exports the prolonged-sound mapping. The deterministic
Python builder deduplicates complete records and emits no machine paths/times.
Ordinary Rust builds load the checked-in weight data without Java, Python,
network, weight-generation build steps or runtime caches. The derived FST reader
below has its own upstream codec-table build script, not a weight generator.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| general-legacy.bin | 765112 | `7045a80ff0942794345c9ad08fb335637b6c421820ffe7f33cb91ed452238617` |
| general-2010.bin | 1119560 | `13e702d03d2f89a28e59a8b48072463f32073a9606ec023843e872051f3ec8a9` |
| kana-vowels.bin | 256 | `0e3d5e0c512fcea8b3a09d6f8ccd11241d35f042f4ff6098d0b2c136f9137ac9` |
| like-legacy.fst | 33231 | `5c63462ffb2a5062e53db01610e4e6440909b2ad149241024fa0bf9ea7271bf6` |
| like-2010.fst | 75954 | `5a6d7d415a3cb502ea5d23e39e4d5db088019aedde633dcec887a57c759bbe37` |

The `general-*.bin` normalized format is `TCGW`, little-endian u32 version 1, 65,536 little-endian u32
offsets relative to the record region, then deduplicated records. Each record
has twelve u8 fields: kind, character units, primary entries, placeholder,
modifier, kana flag, prolonged flag, four payload lengths and reserved zero.
Payloads follow in primary/secondary/unprintable/tertiary order. The Rust consumer
uses endian-explicit u32 reads and borrowed alignment-one zerocopy records.
The kana artifact has 256 u8 mappings. These are private normalized formats,
not native Access file structures or general-purpose database sort tables.

Regenerate with the exact reviewed binary artifact:

```sh
python3 docs/testing/build_general_weights.py --jar PATH/TO/jackcess-5.0.4.jar --output crates/engines/tablecraft-ace/data/general
```

Add `--vectors-output crates/engines/tablecraft-ace/src/collation/key/tests` to
regenerate both reference-key artifacts too. Regeneration into a fresh temporary
directory reproduces the five original weight/key-vector artifacts byte-for-byte.

`GeneralKeysOracle.java` independently generates the encoder's string vectors
and exhaustive per-block SHA-256 key fingerprints. Matching those vectors is
independent algorithm evidence using the same reviewed weight data; it is not
an independent source for those weights or native Microsoft Access qualification.

The two `like-*.fst` files are original derived sets, not copied native indexes.
For every raw UTF-16 unit, the complete current General encoder supplies a key
with widened positions and retained trailing spaces. Sorted distinct keys are
serialized with fst 0.4.7's version-3 set builder. The legacy set contains 35,886
keys; General 2010 contains 56,356. Both retain the existing weight-data notices.
No Unicode normalization, OS locale or physical prefix/digest supplies equivalence.
Regenerate/review these artifacts whenever weights, literal-key encoding or the
FST serialization policy changes; they are private derived data, not a stable
public interchange format.

```sh
python3 scripts/regenerate_general_like.py
python3 scripts/regenerate_general_like.py --write
```

The default verifies byte-for-byte without replacing artifacts (Cargo may build
ordinary target files). `--write` replaces only the
two artifacts; independent Rust hash expectations are never rewritten. The
engineering generator runs as an explicitly ignored Rust test and requires no
Java or native Access. Ordinary tests verify FST CRCs, both SHA-256 values, all
65,536 source-unit keys and exact distinct counts, proving no extra keys exist.
