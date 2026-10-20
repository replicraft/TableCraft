# General weight data

These are normalized Apache-2.0 Jackcess 5.0.4 weight tables for General legacy
and General 2010, including surrogate-unit weights and the kana vowel mapping.
Read [NOTICE](NOTICE) and [LICENSE-APACHE-2.0.txt](LICENSE-APACHE-2.0.txt). The Rust
encoder in `src/collation/key/encode.rs` and `tail.rs` is also an Apache-2.0
adaptation. Other original TableCraft code retains its MIT license; the package
SPDX expression is `MIT AND Apache-2.0` to retain both obligations.

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
Ordinary Rust builds load the checked-in static data and require no Java, Python,
network, build script or runtime cache.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| general-legacy.bin | 765112 | `7045a80ff0942794345c9ad08fb335637b6c421820ffe7f33cb91ed452238617` |
| general-2010.bin | 1119560 | `13e702d03d2f89a28e59a8b48072463f32073a9606ec023843e872051f3ec8a9` |
| kana-vowels.bin | 256 | `0e3d5e0c512fcea8b3a09d6f8ccd11241d35f042f4ff6098d0b2c136f9137ac9` |

Normalized format: `TCGW`, little-endian u32 version 1, 65,536 little-endian u32
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
directory reproduces all five tracked artifacts byte-for-byte.

`GeneralKeysOracle.java` independently generates the encoder's string vectors
and exhaustive per-block SHA-256 key fingerprints. Matching those vectors is
independent algorithm evidence using the same reviewed weight data; it is not
an independent source for those weights or native Microsoft Access qualification.
