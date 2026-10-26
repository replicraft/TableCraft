# Native encryption

`tablecraft-ace::inspect_encryption` and `tablecraft encryption inspect PATH`
provide inert page-zero envelope inspection. Separate library APIs now validate
qualified Agile descriptors, verify passwords and decode explicit data pages.
Qualified Agile sources now support native table/query reads with global
`--password` / `-p`. Qualified add/remove/change-password copies now work. Ordinary encrypted table updates remain blocked. Raw header inspection does not qualify encoded table layouts
without an explicit authenticated source context.

The inspection distinguishes zero-key pages, nonzero Jet 4 file encoding and
ACE Office encryption envelopes. Zero-key pages do not establish the absence of
a legacy database password. Jet encoding, database passwords and workgroup
authorization are separate concepts. Neither inspection command requests a
password. Basic `inspect PATH` adds page encryption status and its envelope type;
the encryption subcommand owns the detailed version/family/length/flags report.
The old `inspect --encryption` flag is removed. "Not performed" distinguishes
metadata inspection from password verification and page decoding.

Table/row/query and detailed native inspection commands prompt once when a
qualified encrypted source needs credentials, `-p` is absent, and standard input
is a terminal. `rpassword` reads hidden input from the controlling terminal; the
CLI immediately wraps its returned String in `Zeroizing`. Plain sources and
unsupported/malformed descriptors never prompt. A wrong or empty response is
attempted once; no automatic retries or fallbacks occur. Noninteractive input
requires explicit `-p`; it never consumes piped records as a password. Prompt
failure/EOF publishes no rows. Typed prompt input is not a shell command and
stays out of shell history; explicit `-p` values may be recorded in history or
process arguments.

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

## Profile and codec coverage

Encryption schemes are selected from the native header/envelope, not the Access
product's marketing version. ACE 14/15 files do not each require a separate
cipher implementation merely because Access releases differ.

| Native family | Working encoded reads | Evidence / remaining limits |
| --- | --- | --- |
| Jet 4 MDB | Native 32-bit-key RC4 page encoding | Original encoded fixture: all 28 data pages and two exact table/query rows match Jackcess Encrypt; key resides in header, no password verifier |
| ACE 2007 / code 2 | Agile AES-CBC; Office Binary RC4; CryptoAPI RC4/SHA-1; standard and compatibility AES-ECB/SHA-1 | Original code-2 CryptoAPI file plus deterministic shared-layout routing / independent vectors |
| ACE 2010 / code 3 | Same declared Office codecs | Original Agile and compatibility AES files; exact independent pages and rows |
| ACE 2016 / code 5 | Same declared Office codecs | Original modern-history files encrypted by both reference and TableCraft writers; exact pages/rows/SIDs and copy recovery compare independently. Legacy vectors/routing remain separate; native Access-authored comparison is pending. |
| ACE 2019 / code 6 | Same declared Office codecs | Same original encrypted-history comparisons as code 5, additionally including Extended-date histories; native Access-authored comparison is pending. |
| Jet 1/2/3 | Not qualified | No usable local table read profile; do not infer support from RC4 reuse |

`EncryptedSource` selects qualified codecs and grants the same explicit native
page context as `AgileSource`; raw Read/Seek retain original ciphertext and
`NativeImage::capture` still rejects encoded images. Remove/change-password copies now consume the same codecs under the explicit
rewrite contracts below. New ACCDB encryption uses Agile AES-256/SHA-512;
Jet 4 creation uses its native recoverable encoding and password field.

Legacy Office supports version 1.1 Binary RC4/MD5 with a 128-bit derived key;
version 2/3/4.2 CryptoAPI RC4/SHA-1 with explicit byte-aligned 40–128-bit keys
(provider type 1); and AES-128/192/256 ECB/SHA-1 (provider type 24). Standard AES
uses 50,000 hash iterations; the positively identified compatibility form with
the AES flag absent uses zero iterations. There is no retry of alternative keys
or schemes after a bad password. RC4 40-bit keys are zero-extended to 16 bytes.
Provider-derived default widths and flag-selected algorithm defaults are bounded
and explicit; unsupported provider/hash/flags and malformed lengths fail before
asking for credentials. No external cryptographic provider is invoked.

Legacy Jet encoding is reversible with the key in its header and needs no
password. Legacy MDB password fields and workgroup authorization remain separate
and are not authenticated by this page codec; `-p` does not turn encoding into a
password-based cipher. RC4/MD5 dependencies exist only for native legacy
interoperability. Only explicit native Jet 4 add operations select RC4 encoding
for new outputs; new ACCDB encryption uses Agile AES-256/SHA-512.

Extensible/external providers, unknown schemes, wider RC4 providers/keys, arbitrary
Office algorithms, general encrypted table edits and
native Access save/reopen remain unsupported or unqualified. Consequently,
Header recognition alone does not establish complete profile or native Windows compatibility.

Original JCE/Bouncy Castle vectors in `LegacyEncryptionVectors.java` cover Jet
encoding, Binary RC4, CryptoAPI 40/128-bit keys, and standard/compatibility AES at
all three widths using the public Unicode password `A😀é`. Engine tests verify
those independent constants, all qualified header families, credential and
length failures, descriptor truncation, unsupported parameters, source-byte
preservation, invalid page ordinals, cancellation and budget atomicity.
`compare_legacy_encryption.py` checks original raw-page hashes, exact rows/SELECTs/
counts, credentials, unchanged sources and derivative routing through codes
2/3/5/6. Original creation recipes/native Access builds remain unverified.

Primary format references (reviewed locally at the already recorded revisions):
[JetCryptCodecHandler](https://jackcessencrypt.sourceforge.io/apidocs/com/healthmarketscience/jackcess/crypt/impl/JetCryptCodecHandler.html),
[ECMAStandardEncryptionProvider](https://jackcessencrypt.sourceforge.io/apidocs/com/healthmarketscience/jackcess/crypt/impl/office/ECMAStandardEncryptionProvider.html),
[RC4CryptoAPIProvider](https://jackcessencrypt.sourceforge.io/apidocs/com/healthmarketscience/jackcess/crypt/impl/office/RC4CryptoAPIProvider.html)
and [MS-OFFCRYPTO](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-offcrypto/).
No upstream implementation code was copied into TableCraft.

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
[Dependency records](../../development/DEPENDENCIES.md#native-agile-encryption-20-october-2026)
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
The normal CLI separately accepts global `--password`/`-p` before or after native
read commands. Tables, row pages/all-table browsing, native queries/saved SELECTs
and detailed inspection share one authenticated source and cumulative allowance.
Header/encryption-only inspection stays inert and needs no credentials. Supplied
CLI password strings are zeroized when dispatch ends and omitted from diagnostics;
command-line/host copies remain outside that guarantee. Plain sources retain
existing behavior. Native UPDATE copies stay blocked on encoded inputs. Explicit
remove/change-password tools now publish separate output paths; add remains next.

## Raw and decoded source boundary

`NativeReadSource: Read + Seek` adds explicit header qualification and native-page
decoding hooks. File, Cursor, SharedFile and offline read transactions use plain
defaults. Custom raw readers add an empty trait implementation. Native APIs and
Access browsing/query adapters now require this trait; raw header/encryption
inspection and whole-image capture still accept general Read + Seek.

`AgileSource::open` verifies a password once and binds the decoder to its owned
source. Raw Read/Seek still return original ciphertext. Only native page readers
invoke decoding, under caller budgets/cancellation. A catalog header retains
`encoded_pages=true` plus a private decoded read context; parsing or inspecting
raw bytes does not grant that context. Geometry changes are rejected, and the
host must keep the source stable/offline. The source has no Write interface and
caches no plaintext pages; snapshots/cursors retain decoded data under their
ordinary lifetime rules. Whole-image preservation still sees ciphertext and
rejects unsupported encoded capture. No zero-key plaintext view replaces raw I/O.

ACE 2010 ordinary-table geometry now has `ReadProfile::Ace2010`, covering the
header used by the original Agile inputs. Qualified ACE sources accept subversion
zero or one, including plain outputs after encryption removal; Jet 4 requires zero. This does not enable Large Number,
Extended DateTime, cached calculations or advanced/index variants merely because
another ACE family supports them. Native Access/Windows qualification remains
separate from independent ordinary-table evidence.

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
in [FIXTURES.md](../../testing/FIXTURES.md#encryption-envelope-reference-inputs). Run the local
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
broader codec/field/profile coverage and Access/Windows evidence.

`compare_encrypted_browsing.py` additionally compares exact CLI row values
(including NULL/text), inventory/schema/counts, ordered queries, aggregates,
all-table browsing and credential failures with independent encrypted cursors:
one row in the original 2007-named input and seven in the 2013-named input. Both
retain their hashes. Original deterministic header-retagged encrypted derivatives
exercise four ACE read profiles in CI, including Unicode credentials, raw-byte
preservation, cursor batches, encoded-image refusal and resource failures. These
derivatives test routing/shared layouts, not native-authored file variants.

## Remove encryption and change password

```sh
tablecraft encryption remove source.accdb --output unlocked.accdb
tablecraft encryption change-password source.accdb --output changed.accdb
tablecraft encryption remove source.mdb --output unlocked.mdb
```

These commands preserve their source and require a fresh explicit output path.
Global `-p` supplies the current password; otherwise interactive commands prompt
once with hidden input. Source geometry/scheme and current credentials are checked
before asking for a replacement. Change-password uses `--new-password` or two hidden
new-password/confirmation prompts. An explicitly supplied command-line password
can appear in shell history/process arguments. Noninteractive callers must
supply needed credentials; piped records are never consumed as passwords.

Removal/change work for all qualified read families (Jet 4 and ACE codes
2/3/5/6), and every implemented cipher: Agile AES-CBC; Binary RC4/MD5; CryptoAPI
RC4/SHA-1; standard and compatibility AES-ECB/SHA-1. Existing key widths, hash
algorithms, iteration counts and native version/subversion bytes are retained.
ACE subversion one is now also a qualified plain read layout: Access preserves
that byte after removing encryption. It must not be downgraded to appease a
reader gate. Jet 1/2/3 and external/unqualified providers remain unsupported.

`rewrite_encryption` produces an explicit unpublished `EncryptionCopy`, separate
from normal edit plans and journals. This image API retains its **64 MiB image
cap**. `rewrite_encryption_to` instead transforms pages into an empty, unpublished
host-owned destination within the **2 GiB native read-source ceiling** and the
caller's cumulative allowance. It retains zeroizing page buffers and budgeted,
sorted SID patches rather than a complete image. The CLI uses this page path;
its existing 512 MiB cumulative byte allowance can refuse a copy before the native
file ceiling. The allowance includes page reads/writes, metadata and crypto work;
final cancellation prevents success. Owned
image/key/SID buffers use zeroization. No macros, VBA, expressions or external
providers are executed. Source stability/offline access remains a host contract;
this is not shared-file locking, cross-client coordination or a recovery journal.

Removal decrypts data pages and clears the native key, date-masked password field
and (for Office) the entire `0x299..0xe00` encryption-info region. Changing a Jet
password changes its date-masked UTF-16 field. Both require a new native SID mask:
`MSysACEs.SID` and `MSysObjects.Owner` are converted using old/new RC4 keystreams.
The shared checked record geometry follows bounded relocations, skips deleted
roots, preserves NULL/empty fields and rejects aliases/overlapping patches.
Only same-width Short Binary values on **unindexed** affected columns are
qualified. Unsupported SID layouts/indexes fail before publishing an output.
Jet replacement passwords are nonempty, at most 20 UTF-16 units, and cannot
contain NUL; Office credentials retain their 255-unit bound. Legacy Jet passwords
are checked for these mutations even though ordinary Jet encoding reads use the
stored page key without authenticating that separate legacy field.

Agile password changes generate a fresh password salt/verifier and rewrap the
same data key. Data-page ciphertext, native password field/encoding key, data
parameters and any existing integrity fields remain unchanged; no SID remasking
is needed. Existing file-HMAC verification remains unimplemented. Legacy Office
schemes derive data keys from passwords, so they receive fresh salts/verifiers
and their data pages are re-encrypted. Their native SID mask remains unchanged.
Other decoded data and opaque object/VBA content are preserved.

The CLI stages a complete output in a named temporary file beside the destination,
syncs it, then uses `persist_noclobber`; existing paths are never overwritten.
Handled failures clean up staging files. File synchronization and fresh-path
publication are not a directory-durability/crash-recovery guarantee, and forced
termination may leave temporary files requiring cleanup. Unix outputs use the
temporary-file library's private creation mode; Windows ACL behavior needs its
own platform checks. [Dependency review](../../development/DEPENDENCIES.md) records getrandom and
tempfile roles; temporary-name randomness is not encryption-key generation.

### Independent rewrite evidence

`compare_encryption_rewrites.py` compiles the original read-only
`NativeSecurityOracle.java` and `EncryptedPagesOracle.java` drivers, then compares
both operations against Jackcess 5.0.4, Jackcess Encrypt **5.0.2** and Bouncy Castle
1.86. The new crypt jar is Apache-2.0, SHA-256
`6dbb05c767a15edd33fad0ca1d054767e71992748f2cba11d227bac00d4e648d`;
its source artifact is 81,554 bytes, SHA-256
`a9d14c81856cb3ead4da2001398f20e509b53b29c234dc127d296fa1533f1258`.
The source, POM, compatible core dependency and notices were reviewed. The older
5.0.0 raw-decoder baseline is retained separately. No third-party implementation,
JAR or fixture bytes are copied into this repository.

All **43 inputs / 86 rewrite outputs** independently reopen with exact table
rows and decoded SID digests preserved, current/new native passwords agreeing,
old passwords rejected on mutations, original bytes unchanged and existing
outputs protected. Inputs comprise seven original upstream files (including two
Jet password fixtures with different creation dates) plus 36 independently
JCE/BC-encrypted derivatives: Binary RC4, CryptoAPI 40/128-bit RC4 and
standard/compatibility AES-128/192/256 across codes 2/3/5/6. The optional output
directory argument to `LegacyEncryptionVectors.java` creates this corpus from an
original four-row Jackcess fixture; its fixed public crypto inputs are test-only.
Agile mixed SHA-1/256/384/512 and AES-192 rewraps additionally preserve the existing
independent JCE vectors and original ciphertext. Original Agile outputs retain
all data-page ciphertext. These are independent interoperability tests, **not
native Microsoft Access/Windows save qualification**.

Primary rewrite references:
[OfficeEncryptionHandler](https://jackcessencrypt.sourceforge.io/apidocs/com/healthmarketscience/jackcess/crypt/impl/OfficeEncryptionHandler.html)
and [SidRemasker](https://jackcessencrypt.sourceforge.io/apidocs/com/healthmarketscience/jackcess/crypt/impl/SidRemasker.html).
Add-encryption now supplies fresh file material and native version/password/SID
transforms under the separate creation contract below.

## Add native encryption

```sh
tablecraft encryption add plain.accdb --output encrypted.accdb
tablecraft encryption add plain.mdb --output encoded.mdb
```

New passwords prompt with hidden confirmation. `--new-password` supports
noninteractive input and has the same history/process-argument exposure as `-p`.
An unencoded Jet source with a database password requires its current credential
through `-p` or a hidden prompt before requesting the replacement. Already encoded
inputs fail explicitly; use change-password to retain their current cipher.

`EncryptionRewrite::Add` works through either output API. The image API retains
its **64 MiB** capture limit; page copies use the bounds above. Credential
preflights validate the source independently of the output representation and do
not promise that a later image capture fits. Sources and existing outputs stay
untouched. OS randomness
supplies the encoding key and, for ACCDB, a 256-bit data key, independent data/
password salts and verifier. A zero encoding-key sentinel fails explicitly.
Affected unindexed SIDs use the same checked remasking as removal; unsupported
layouts block publication. No ordinary table-write authorization is granted.

New ACCDBs use Agile 4.4 AES-256-CBC/SHA-512, 100,000 password-hash iterations,
16-byte salts/blocks and native per-page IVs. The date-masked native password
marker repeats the encoding key's low byte. ACE 2007 upgrades to code 3/subversion
1 for Agile; later native family bytes stay unchanged. The profile emits no file
HMAC and does not authenticate ciphertext. Shared verifier/key-wrapping code also
serves change-password; page transforms retain atomic failure publication.

Jet 4 sets its native password and fresh 32-bit-key RC4 page encoding. **This is
recoverable native encoding, not modern password-derived encryption:** the page
key is stored in the header. Jet passwords remain nonempty, at most 20 UTF-16
units and without NUL; ACCDB retains its 255-unit limit without normalization or
truncation. Jet 1/2/3, external providers and signed-package rewrites stay blocked.

CI controls cover all five read families, distinct fresh outputs, exact add/remove
images (allowing the ACE 2007 version upgrade), credential/output protections and
cancellation through final publication. `compare_encryption_rewrites.py --check-add`
also checks independent rows/SELECT, decoded SID digests and old Jet credentials.
The original `RemoveAddedEncryptionOracle.java` driver removes protection from
fresh test outputs with Jackcess Encrypt and must recover the complete expected
plaintext image. Corpus originals are untouched. This independent-library
evidence does not establish Microsoft Access save/reopen compatibility.

All **43 inputs** pass this creation comparison: seven original encrypted or
Jet-password inputs and 36 independently generated legacy Office/profile inputs.
The unencoded password-set Jet originals also exercise current-credential refusal
before replacement. Each added output independently reopens, and its independent
removal recovers every expected plaintext byte, allowing only the documented
ACE 2007 version upgrade.

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

## Rewrite measurements

Hyperfine 2.0.0 on the same local Apple Silicon release build, two warmups and
eight runs with a fresh output per run, measured complete CLI commands including
source/current-credential validation, rewriting, staged writing and file sync:

| Original public fixture | Remove | Change password |
| --- | --- | --- |
| AES-128/SHA-1, 100,000 iterations | 27.9 ± 1.9 ms | 37.6 ± 1.7 ms |
| AES-256/SHA-512, 100,000 iterations | 48.6 ± 1.5 ms | 67.1 ± 2.1 ms |

These are **milliseconds per complete copy operation**, with explicit test
password flags, on roughly 0.4 MiB inputs. They do not measure interactive typing,
large-file scaling, per-row query cost, cold storage or another platform. There
is no prior rewrite baseline or claimed optimization speedup.

### Creation measurements

The same release/Hyperfine setup (two warmups, eight runs, fresh output, file
sync included) measured complete add operations with a public 17-character ASCII
password:

| Plain input | Complete add copy |
| --- | --- |
| Removed original Agile example, 430,080 bytes | 27.4 ± 1.0 milliseconds |
| Removed original encoded Jet example, 118,784 bytes | 8.5 ± 1.3 milliseconds |
| Registered ACE 2007 example, 3,387,392 bytes | 46.1 ± 8.2 milliseconds |

The first pilot used a password beyond Jet's 20-unit limit, so its Jet warmup
correctly failed. Its partial ACCDB timing was retained separately and excluded
from this complete run. Raw JSON retains all eight samples per accepted case;
the slower large-file sample is included. These are warm local copy costs,
not per-row timings, a comparative speedup or native Access/platform evidence.

## CLI inspection and terminal acceptance

The CLI test suite passes 121 tests across 23 suites after moving detailed
inspection; the focused inspection suite also passes after additional summary
and removed-flag assertions. CLI all-target/all-feature Clippy, formatting/diff
checks, dependency boundaries and ten policy tests pass. The four pinned original
encryption envelopes and two original Agile table/query comparisons still match
and preserve source hashes.

`docs/testing/check_password_terminal.py --cli PATH --directory INPUT_DIRECTORY`
uses a Unix pseudo-terminal and the existing pinned public 2007-named fixture.
On macOS it verifies successful and incorrect-password interactions, echo disabled
while entering credentials and restored afterward, exactly one prompt, no secret
in terminal output, and unchanged source bytes. This console check needs actual
terminal access; sandbox console access is restricted. Windows console execution
and native Access comparisons remain unavailable, not claimed.


## Encrypted modern history interoperability

The [modern encrypted corpus](../../testing/ENCRYPTED_MODERN_HISTORIES.md) now covers
real 2016/2019 layout histories encrypted independently by Jackcess Encrypt and
TableCraft, with 19,902 rows per file. Generation passes 98 controls, including
6,010 decoded-page comparisons, exact modern-field rows/indexes/queries, SID
preservation, credential/limit refusals, ciphertext-preserving password changes
and complete independent plaintext-image recovery. Originals remain unchanged.
These fresh-copy checks do not enable ordinary table writes or qualify native
Microsoft Access behavior.

The rewrite runner now enforces the already reviewed 5.0.2 JAR and exact core/BC
hashes before compiling the SID/removal helpers. The separately recorded 5.0.0
raw-decoder artifact lacks those APIs and fails clearly when supplied there.
No runtime dependency, copied library source or binary fixture is added.

## Page-copy preservation and failure boundary

The page-copy path uses the same qualified codecs, password envelopes and SID
layout restrictions as image copies. It verifies every SID before-image on its
page before applying that page's patches, rejects overlapping/cross-page or
unconsumed patches, and checks original page-zero bytes and file geometry before
output and after the final page. These checks do not provide live-file coherence;
the input must remain stable/offline throughout the operation.

Agile password changes retain every data-page ciphertext byte. Other operations
decode, remask qualified SIDs and encode one page at a time. Existing ACE 2007
Agile creation still promotes the header to the ACE 2010 compatibility envelope;
removal retains that header change. No format downgrade is inferred.

The library destination must be empty, distinct from the source and unpublished.
Seek/write/cancellation/budget failures can leave an unusable prefix; callers must
discard it. The CLI discards failed temporary files, synchronizes successful ones
and publishes with no-clobber persistence. It does not replace existing files or
supply a transaction journal. Native Access save/reopen remains unqualified.

The [large-file replay](../../testing/LARGE_ENCRYPTION.md) passes 92 commands over
unchanged 77 MiB V2016/V2019 originals. Independent getters/SID/page readers and
independent encryption removal verify added/changed/removed candidates. Every
recovered full image matches its original digest, and all data ciphertext stays
unchanged across Agile password replacement. The report keeps its two
size-limit baseline refusals separate from current implementation successes.
This does not qualify the full native file ceiling or native Access saves.
