# Encrypted ACE 16 modern histories

The original [comparison runner](compare_encrypted_modern_histories.py) reuses the
registered [modern scalar histories](MODERN_SCALAR_HISTORIES.md): 19,902 exact rows
per ACE 2016/2019 layout, Large Number values, 2019 Extended dates, later-added
fields, Boolean/null-mask growth, deleted roots and grown Memo/OLE payloads.
It creates fresh Agile copies with both Jackcess Encrypt and TableCraft, then
compares both producers against the independent Python model and getters.
No database original changes or active database content executes.

The original [reference creation driver](AddEncryptionOracle.java) invokes the
fresh-output EncryptionUtil overload. [Encrypted getters](ModernEncryptedOracle.java)
open read-only, disable expression evaluation, use civil dates and reuse the
existing tagged row encoding. TableDataOracle's row helper is now package-visible;
its standalone main, encoding and plaintext JAR requirements remain unchanged.
Passwords in the runner are explicitly public test inputs, with Unicode and
supplementary characters. Reference programs receive them through stdin.

## Independent comparisons

For each of the four encrypted files, the runner checks:

- Every stored modern-history value and native sequence, plus complete CLI
  progressive rows and buffered query results against the independent model.
- Complete Large Number index order, exact ID/key values and Extended date
  projections/order where the layout supports that type.
- Every decoded data-page SHA-256 through both Jackcess Encrypt and the Rust
  agile_pages engineering reader: 1,330 pages per 2016 input and 1,675 per 2019
  input, totaling 6,010 page comparisons across both producers.
- Decoded native SID digests before and after password changes.
- Inert encryption inspection, missing/empty/wrong credentials and explicit byte
  exhaustion, with no buffered query stdout on failure.
- Password changes that preserve all data ciphertext; independent reopening with
  the new credential and refusal of the previous one.
- TableCraft removal and independent reference removal each recovering every
  byte of the registered plaintext original, including the native header/SIDs.
- Unchanged plaintext and encrypted input hashes after all operations.

Fresh generation passes 98 controls. Registered reuse passes 82 controls,
skipping sixteen creation operations while retaining all read/page/SID/image/
credential checks. Copy management is the existing
bounded fresh-output operation, not newly enabled table writes. Ordinary native
writes and encryption-management limits remain unchanged.

## Reviewed engineering tools

The shared [artifact guard](encryption_references.py) checks exact SHA-256 values
for Jackcess 5.0.4, Jackcess Encrypt 5.0.2 and Bouncy Castle 1.86 before invoking
reference code. The existing rewrite comparison now uses that guard as well.
The detailed encryption contract already recorded 5.0.2 for SID/removal helpers;
the general dependency summary still named only the older 5.0.0 raw-decoder
baseline. The summary is corrected. Supplying the reviewed 5.0.0 JAR now fails
clearly before compiling these helpers: it lacks the required APIs.

Both crypto JARs were restored to ignored persistent storage and matched their
previously recorded hashes. No JAR, database, reference implementation source or
new Rust dependency is committed. The older 43-input rewrite corpus was not
rerun in this batch. The new corpus instead exercises modern types and real
ACE 16 layouts without retagging old encrypted headers.

## Reproduction and scope

```sh
python3 docs/testing/compare_encrypted_modern_histories.py \
  --cli target/debug/tablecraft --pages target/debug/examples/agile_pages \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --crypt-jar .cache/tablecraft-fixtures/jackcess-encrypt-5.0.2.jar \
  --bc-jar .cache/tablecraft-fixtures/bcprov-jdk18on-1.86.jar \
  --directory .cache/tablecraft-fixtures/modern-scalar-histories \
  --output .cache/tablecraft-fixtures/encrypted-modern-histories
```

Generate into a new output directory with --generate. Existing directories and
output files are refused. Random encryption material changes fresh hashes;
review/register the resulting identities before reuse. Files and compiled
engineering helpers remain under ignored persistent repository storage.
[Provenance](FIXTURES.md#encrypted-modern-history-inputs) records the observed
files. An initial runner failure used a duplicate byte-limit flag; its partial
local generation/log remains excluded from this registered corpus.

This establishes independent engineering interoperability for these files. It
is not native Microsoft Access save/reopen, SQL/optimizer/locale qualification,
a cryptographic audit, authenticated data-page integrity, arbitrary encrypted
large-file support, shared-file coherence or authorization for table writes.
Windows comparisons remain pending under the user's deferred test plan.
