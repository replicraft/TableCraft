# Persisted signed complex child identities

The complex link reader previously reused its positive foreign-key validator for
child identities. That rejected otherwise readable multivalue, attachment and
Memo-history snapshots containing negative or zero child IDs. Independent reopened
typed getters read those exact IDs and their associated payloads successfully.

The link reader now separates the roles. Assigned conceptual parent keys and
stored child foreign keys remain strictly positive. Child identities preserve the
complete signed Int32 domain, including zero, -1 and both boundaries. They must
still be present Int32 values and unique within their selected flat snapshot;
NULL IDs, duplicate IDs, orphans, ambiguous parents and unsupported declarations
retain their existing errors. The two typed record accessors follow the same
signed identity contract. No value becomes absent because its ID is negative.

## Independent stored-read evidence

The original [producer](SignedComplexIdentityFixture.java) creates fresh V2016 and
V2019 databases with two ordinary parent rows. One parent has eight members of
each of three independent collections; the other has none. The identities, in
physical stored order, are `1, -2147483648, -7, -2, -1, 0, 7, 2147483647`.
The multivalue member at ID -1 has a NULL payload, which remains a present child.
Attachment labels and historical text distinguish each identity. Encoded attachment
envelopes and explicitly decoded file contents remain separate representations.

The producer first uses typed public collection helpers to create ordinary values,
then uses public flat-table AutoNumber insertion controls to author the remaining
IDs. Pinned reference implementation metadata getters discover actual flat-table,
identity and foreign-key columns. No bytes or declaration flags are patched.
Expression evaluation is disabled; no imported action, formula or code executes.
This authoring route establishes a useful independent stored-read corpus. It does
**not** establish that Access itself generates or accepts every such child ID,
or authorize TableCraft to create or mutate those records.

The [oracle](SignedComplexIdentityOracle.java) reopens originals read-only and
compares physical flat-row IDs, actual positive references and public typed
multivalue, attachment and version getters. Every persisted typed identity has a
non-NULL physical RowId, including -1 and zero. The pinned reference helper
`ComplexColumnInfoImpl.INVALID_ID` separately contains numeric -1 with a NULL
RowId. It is an insertion/update helper sentinel, not a rule that stored numeric
-1 means absence. The reference's separate `INVALID_FK` helper does not relax the
positive parent/reference profile. No zero identity sentinel was observed in the
reviewed classes. The engine never uses those helper objects or sentinel equality.

Reference version getters sort timestamp ties by descending identity. The replay
maps their values by identity and obtains physical order independently from flat
getters; TableCraft continues to publish flat snapshot order without synthesizing
a latest version or freshness claim.

## Accepted replay and focused checks

The [runner](compare_signed_complex_identities.py) passed 88 controls on 24 October
2026 against the immutable CLI SHA-256
`4f3049ae09612ea9c54ecbf7a6bffff62b1fd2d7e22026619e821609674903eb`:

- Two reopened getter/association controls and ten collection inspector controls.
- Sixty-four exact query comparisons across both files and ordinal/database
  comparison modes, including member projection, nullable payloads, ordinary
  parent counts, member counts, empty membership, stored order, encoded and
  decoded FileData, and actual discovered history member names.
- Twelve cumulative native/query/output resource refusals with no stdout.

The [accepted report](results/signed-complex-identities-2026-10-24.json) pins fourteen
unchanged original input, recipe, oracle, runner, implementation, JAR and executable
hashes. Early harness attempts used an assumed history timestamp name and incorrect
multivalue predicate-count expectations. Those failed runs produced no accepted
report and are excluded; final comparisons use discovered names and the existing
documented parent/member predicate contracts. The preliminary positive/negative
parent and child authoring probes are also excluded from the passing count.

Focused engine targets pass 33 tests and adapter targets pass 23 tests. New
synthetic controls cover signed boundaries, duplicates, strict parent/reference
positivity, full and metadata-only attachment snapshots, history order, exact
encoded/decoded payloads, presence and complete-cache reuse. Existing targets
retain NULL, orphan, quota, cancellation and payload-pruning checks. Scoped strict
Clippy passes for all five targets; scoped rustfmt, Python compilation and
whitespace checks pass.

See [fixture provenance](FIXTURES.md#original-signed-complex-child-identity-inputs).
These are bounded independent-reader results, not native Access SQL, native
application qualification, concurrency, mutation or save/reopen evidence.
