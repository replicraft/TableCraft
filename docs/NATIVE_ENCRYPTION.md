# Native encryption

`tablecraft-ace::inspect_encryption` and `tablecraft inspect PATH --encryption`
provide inert page-zero envelope inspection. **Encrypted table reads, password
verification and page decryption are not implemented.** The normal read-profile
selector still rejects nonzero page-encoding keys.

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
does not read encrypted data pages or change source bytes. No dependency changed.

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

This checks metadata, continued encoded-read refusal and unchanged source hashes.
It does not decrypt anything or establish actual Access compatibility. The
upstream fixture names do not prove an ACE 16 file layout or a recorded creating
Access build. Future read qualification needs full cipher/KDF validation,
password verification, correct per-page transforms and independent exact-row
comparisons, followed by native Access/Windows evidence.
