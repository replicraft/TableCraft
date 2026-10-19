# Jet 4 current-profile qualification

Updated 19 October 2026. This completes a contained extension of the shared
bounded read/check profile, not full Jet 4 compatibility. ACE 16 remains the main
long-term compatibility target. Reliable reads and queries for both ACE 16 and
Jet 4 are the immediate priority. Windows/native Access qualification is
pending; independent comparisons cannot establish it.

## Applicable common scope

Jet 4 already uses the common bounded unencoded catalog/schema/scalar readers,
property and relationship inspection, qualified indexes, consistency/rule checks,
saved-query fragments and owned local SELECT reconstruction. Existing offline
Int32 replacements retain their preservation/constraint gates. Global resource,
cancellation, unsupported-carrier and stable/offline input limits still apply.

This pass adds complete exact Decimal index reads/checks, two-to-ten-component
Decimal tuples and same-precision/scale Decimal foreign-key membership. The
legacy descending sign byte is not complemented: negative groups precede
nonnegative groups. A private physical ordering key models this without changing
returned coefficients, scale, SQL comparisons or SQL ORDER BY. Typed integer seeks
still reject Decimal. Legacy bytes cannot be qualified by retagging ACE files.

ACE 12 shares corrected Decimal encoding and now has fresh independent fixtures
for the same operations. ACE 14/15 remain header-only/unqualified for table reads;
this work does not expand their profiles.

ACE-only attachments, multivalues, calculated fields, table data macros, Large
Number and Date/Time Extended are not Jet 4 parity requirements. General Text/
binary index keys, encryption, replicated/workgroup-security profiles, broader
CRUD/schema mutation, compact/repair, application execution/preservation and
native shared-file safety remain outside the implemented profile. A successful
check means only its reported coverage, not full database integrity.

## Independent evidence

Fresh original V2000 and V2007 files were generated with the previously reviewed
Jackcess 5.0.4 artifact, with expression evaluation disabled. No third-party source
or database bytes were copied into production. [Fixture hashes](FIXTURES.md#jet-4-and-ace-12-current-profile-inputs)
record each original; generated databases remain untracked.

- Single Decimal trees: 6,116 ordered row visits per family; precisions 4/28,
  scales 0/2/4/28, exact 28-digit extrema, NULL/unique/primary checks and corrupt
  selected-root rejection. Legacy descending output deliberately differs from ACE.
- Mixed/ten-Decimal trees: 3,512 ordered visits per family, complete checks,
  residual filters, maximum expanded entries 188/184 bytes and compressed prefixes.
  Jet 4 has nine nodes/90 leaves across ten-Decimal indexes; ACE 12 has nine/92.
  Older fixtures use Int32 in place of the ACE-only Int64 mixed position.
- Relationships: 116 rows, seventeen corrupted copies and two metadata mismatches
  per family. Equality remains exact and independent of physical index order.
- Jet 4 saved SELECTs: 28 compound-join controls and 21 parameter controls match
  the existing owned models; eight malformed copies per set fail without stdout.
  This proves reconstruction, not native SQL execution.
- A fresh Jet 4 UPDATE copy changes one non-indexed Int32 ID beside legacy Decimal
  indexes. Every byte outside its exact four-byte field span remains identical;
  sequential and both index cursors match independently before/after. The original
  remains unchanged. This does not enable Decimal-key mutation or ACE writes.

Existing V2016/V2019 Decimal and saved-query comparisons also pass unchanged.
Focused CI tests cover literal sign/negative-zero/magnitude goldens, per-component
directions, row agreement, empty profiles, exact relationship membership and
unchanged-index preservation around an unrelated Jet 4 edit.

## Reproduction

Use fresh paths; recipes refuse overwrites. Generate DecimalIndexFixture,
DecimalCompositeIndexFixture and DecimalRelationshipFixture with `V2000`/`V2007`.
Their comparison runners retain required `--ace2016`/`--ace2019` regression inputs
and accept optional `--jet4`/`--ace2007` inputs. Build the CLI and ACE `table_data`
example first. See [index contracts](NATIVE_INDEXES.md), [relationship contracts](NATIVE_CONSTRAINTS.md)
and [fixture provenance](FIXTURES.md).

SavedSelectFixture accepts `V2000` with `--compound-joins` or `--parameters`;
compare_compound_joins.py and compare_saved_parameters.py accept `--jet4` alongside
their existing modern controls. Build the Access `saved_select` example as well.
The dedicated [edit comparison](testing/compare_jet4_decimal_edits.py) takes
`--jar`, `--cli`, `--rows` and `--database` pointing to the original Decimal index
fixture, creates only a temporary new copy and independently verifies its results.

## Ownership and next work

Physical codecs/checks stay in ACE. Access maps native data and saved definitions
into shared query/DB contracts. DB has no native format dependency. No storage or
index crate extraction is needed. Next work targets reliable table browsing and
read-only queries for both Jet 4 and ACE 16 ahead of further write expansion, with
native qualification recorded
separately from independently tested byte operations.
