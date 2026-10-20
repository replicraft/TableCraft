# Native encryption

`tablecraft-ace::inspect_encryption` and `tablecraft inspect PATH --encryption`
provide inert page-zero envelope inspection. Separate library APIs now validate
qualified Agile descriptors, verify passwords and decode explicit data pages.
**Encrypted table/query reads and CLI password/rewrite commands are not connected
yet.** The normal read-profile selector still rejects nonzero page-encoding keys.

The inspection distinguishes zero-key pages, nonzero Jet 4 file encoding and
ACE Office encryption envelopes. Zero-key pages do not establish the absence of
a legacy database password. Jet encoding, database passwords and workgroup
authorization are separate concepts. No password is requested or guessed.

For ACE 2007/2010/2016/2019 headers at compatibility subversion zero or one, a nonzero
key selects a bounded descriptor at page-zero offset `0x299`: a little-endian
16-bit byte length followed by the Office version and descriptor bytes. This
area lies outside the fixed header mask. Inspection reads one complete page,
checks the descriptor remains inside it, then reports only version, family,
declared length and applicable outer flags. Jet 3 and other subversions remain
unsupported. ACE 2010 envelope inspection does not qualify its table layout.
Three reference inputs have ACE 2010-family code 3/subversion 1, despite their
2007/2013 filenames. The old-encryption input has ACE 2007 code 2/subversion 0.

| Office version | Envelope reported | Fixed checks |
| --- | --- | --- |
| 1.1 | BinaryRc4 | Version plus salt/verifier/hash storage is present |
| 2/3/4.2 | StandardOrCryptoApi | Outer flags are present |
| 3/4.3 | Extensible | Outer flags are present |
| 4.4 | Agile | Reserved word is `0x40`; a nonempty descriptor body follows |
| Other | Unknown | Bounded version fields are present |

These checks qualify envelope framing only. In particular, Agile XML remains
unparsed, standard/CryptoAPI headers remain unvalidated, and unknown versions
remain visible rather than being guessed. The metadata contains no keys, salts,
verifiers, descriptor XML or connection credentials. Inspecting an envelope does
not authenticate the user or make a cipher configuration usable.

Malformed known prefixes or out-of-page descriptors return
`InspectError::MalformedEncryption`; incomplete source reads retain the existing
truncation/I/O errors. Read bytes and work are charged cumulatively, cancellation
remains explicit, and the CLI buffers its whole report before stdout. Inspection
does not read encrypted data pages or change source bytes.

## Working Agile codec

`read_agile_encryption` reads a bounded descriptor; `AgileEncryption::parameters`
returns non-secret configuration. `unlock(Some(password), budget, cancelled)`
verifies credentials and returns an `AgilePageDecoder`. `None` returns
`PasswordRequired`; an explicitly empty password is attempted. Incorrect
credentials return `IncorrectPassword`. Passwords use UTF-16LE without truncation
or normalization, with a current maximum of 255 code units (`InvalidPasswordLength`
otherwise). The decoder has no Debug, Clone, serialization or write interface.

The profile supports AES-128/192/256 CBC and SHA-1/256/384/512. Password and data
settings are validated separately and may differ. Salt/block widths are currently
16 bytes; hash sizes and encrypted field widths must match. Namespace-aware XML
processing bounds nesting to four elements inside one native page, checks known
fields, accepts XML whitespace in Base64, and rejects DTDs, external callbacks,
unknown algorithms/extensions, multiple key encryptors and certificate providers.
Unsupported profiles return `UnsupportedEncryption`; malformed qualified fields
retain the typed encryption-envelope error. Only UTF-8 XML 1.0 is qualified.

The Office spin-count ceiling is 10,000,000; every iteration charges work and
checks cancellation. One iterated hash feeds three purpose-specific keys.
Maintained RustCrypto primitives, `subtle` verifier comparison and `zeroize`
owned password/key/scratch buffers implement the crypto boundary. Zeroization
does not cover arbitrary host strings, compiler/OS copies, registers or swap.
[Dependency records](DEPENDENCIES.md#native-agile-encryption-20-october-2026)
document features/licenses and the absence of a separate cryptographic audit.

`decode_page` accepts exactly 4096 bytes and an original nonzero in-range ordinal.
Its IV combines the data salt with the little-endian page number XOR the native
encoding key. CBC removes no padding. Temporary plaintext is published only
after the final cancellation check; failures preserve the supplied ciphertext.
Bytes/work remain cumulative-budgeted. The password verifier does not authenticate
ciphertext: callers must validate decoded pages. Optional `dataIntegrity` fields
are shape-checked and reported, but file HMACs are not verified. This API does not
qualify table layouts or implement encrypted writes.

The `agile_pages` engineering example reads a single-line UTF-8 password from
stdin, never argv/environment/logs, and buffers decoded-page SHA-256 digests.
`--verify-only` avoids data pages; `--page N` selects one. It does not export files.
Requested next host work is global `--password`/`-p` for native-read commands,
plus explicit remove/change-password commands writing separate output files.

## References and evidence

The primary native mapping reference is Apache-2.0 [Jackcess Encrypt's
OfficeCryptCodecHandler](https://github.com/jahlborn/jackcessencrypt/blob/77a1d54db8fc71606f391d94adacae7e34b108ca/src/main/java/com/healthmarketscience/jackcess/crypt/impl/OfficeCryptCodecHandler.java),
with its `office/OfficeBinaryDocRC4Provider.java` and
`office/AgileEncryptionProvider.java` fixed-prefix handling. The pinned source
license was reviewed; no Java implementation or binary fixture was copied into
the repository. Our Rust implementation is independently written against the
observed layouts. Microsoft's [MS-OFFCRYPTO specification](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-offcrypto/3c34d72a-1a61-4b52-a893-196f9157f083)
describes Office structures; its package/stream algorithms alone do not establish
the correct ACCDB page mapping.

Four original upstream encrypted inputs exercise two Agile envelopes and two
version-4.2 envelopes, including a nonstandard provider. Their source/hashes are
in [FIXTURES.md](FIXTURES.md#encryption-envelope-reference-inputs). Run the local
comparison with an explicitly supplied upstream `src/test/data` directory:

```sh
python3 docs/testing/compare_encryption_envelopes.py \
  --cli target/debug/tablecraft --directory /path/to/jackcessencrypt/src/test/data
```

This checks metadata, continued ordinary encoded-read refusal and unchanged source hashes.
That envelope runner does not decrypt anything or establish actual Access compatibility. The
upstream fixture names do not prove an ACE 16 file layout or a recorded creating
Access build.

`compare_agile_pages.py` compares every decoded data-page digest with the original
`EncryptedPagesOracle.java` driver using Jackcess Encrypt 5.0.0 and Bouncy Castle
1.86 with the existing Jackcess 5.0.4 baseline. The two additional Maven Central
artifacts have reviewed Apache-2.0/MIT notices and remain engineering-only:

| Artifact | SHA-256 |
| --- | --- |
| jackcess-encrypt-5.0.0.jar | `2961297ed21652da271e7c06d91a6eac1563ff893de0e62190090bddbc16d585` |
| bcprov-jdk18on-1.86.jar | `2af190b300cbb0b35e248ccf5f4a06b6072030aeb3da7a98ec73abe5b4cb371f` |

The original `db2007-enc.accdb` (AES-128/SHA-1) and `db2013-enc.accdb`
(AES-256/SHA-512) agree for all 104 and 94 data pages respectively. Missing,
empty and wrong passwords and invalid page requests fail without partial stdout;
non-Agile inputs remain unsupported. All source hashes stay unchanged. These
results are independent raw-page evidence, not native Access/Windows, table/query,
rewrite or ACE 16 encrypted-layout qualification.

Original deterministic `AgileCryptoVectors.java` JCE vectors additionally cover
all four hashes, AES-192, mixed password/data settings, SHA-1 key padding and a
non-BMP/non-ASCII password without Unicode normalization. Rust tests exercise
actual unlock/decode against those constants. Primitive CBC known-answer tests
use NIST SP 800-38A F.2 first blocks. Remaining read qualification needs source-bound
codec integration, exact table/query comparisons and Access/Windows evidence.

## Opening measurements

On the local Apple Silicon macOS/Rust 1.99 release build, Hyperfine 2.0.0
(`--shell=default --warmup 2 --runs 8`) measured the complete `agile_pages
--verify-only` process with the original public test passwords supplied through
stdin. Both fixtures declare 100,000 hash iterations. AES-128/SHA-1 opening took
9.4 ± 0.4 **milliseconds**, and AES-256/SHA-512 took 19.3 ± 0.7 **milliseconds**.
These include process startup, header/XML reads, password verification and key
recovery. They compare different native algorithms/settings, not an optimization
regression. They do not measure table queries, page throughput, cold storage or
Microsoft Access performance. Hyperfine's 1.6 MiB reported process-memory metric
is not a strict memory quota or complete OS peak-memory qualification.
