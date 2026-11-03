# Fixed updates beside retained Binary indexes

Normal native17 UPDATE can change an eligible unindexed fixed field while retaining
qualified Binary indexes. The positive corpus has an Int32 `Id`, nullable Binary
`KeyValue` and nullable Int32 `Marker`, three physical indexes and six logical
aliases. Updating `Marker` preserves every index byte. This uses the existing
TCNI0004 original-request intent and checked V1 journal services.

## Admission and preservation

The native Source derives its declaration mode from original schema metadata.
Dense field identities, complete raw declarations and aliases, checked index
membership, properties, dependencies, relationships and required-program absence
remain mandatory. Catalog-wide ownership and live LVAL checks exclude INDEX/DATA/
LVAL/metadata/map collisions. Read retention supplies no target-mutation token;
an action Source certificate cannot substitute for canonical UPDATE admission.
The global closed-content route retains its existing declaration policy.

Each actual target still passes ordinary fixed-field and exact carrier checks,
including generated flags, span overlap, duplicate requests, original row bounds
and stale expected values. The independently qualified requests change two
`Marker` cells, clear a presence bit while preserving inactive payload, or retain
the original bytes for a no-op or named empty group. Independent proof reconstructs
the complete image from original requests, then repeats full final Source checks.
All INDEX pages, histories, alias declarations, map records, catalog, unrelated
rows, opaque bytes and EOF remain exact. No allocation or index maintenance is
introduced by this retained-field route.

Indexed `Id` changes now have separate
[qualified mutation and save coverage](NORMAL_RETAINED_BINARY_ID_UPDATE.md),
including no-op and named-empty original requests. The earlier Marker-only result
records its historical refusal before that implementation. Existing Binary/Text
indexed UPDATE and INSERT/DELETE services retain their own target facts,
constraints, ownership and proofs. Source readability alone does not grant those
capabilities or skip a required program.

## Consumers and recovery

The engine ordinary fixed planner, typed retained-session `plan_columns`, SQL
`plan_batch_update` and `query journal-update` use the same normal service.
Native and typed Int32 assignments require exact Int32 or NULL carriers. SQL uses
its existing checked integral-to-Int32 assignment rule; an out-of-range value
refuses before journal creation. For the qualified schema, for example:

```sql
UPDATE BinaryKeys SET Marker=IIf(Id=1,29,70) WHERE Id=1 OR Id=4
UPDATE BinaryKeys SET Marker=NULL WHERE Id=4
```

Original TCNI0004 recovery recaptures Source and reconstructs the expected whole
image before effects. Pending/partial targets roll back; committed mismatches
refuse, and successful completion can be confirmed without target writes.
Retained sessions refresh snapshots, keep their exclusive lease and receipt,
and retire the journal namespace through the existing lifecycle.

## Observed evidence

The [six source images](../../testing/ACE16_RETAINED_BINARY_MARKER_SOURCES.md) and
48 inert SDK reports were qualified before matching writes. The initial canonical
observer refused both originals; after the implementation, the same observer
reports `ADMITTED` for both headers. Both observations remain preserved.

Two direct-proof functions and five native functions pass: ten complete-image
commits, ten literal proof positives, 176 protected-byte forgeries at each proof
and journal boundary, six unique stale proof inputs, eight measured public
resource cases, and 42 recovery contexts with repeated recovery and successful
write-free confirmation. Six committed mismatch contexts refuse without effects.
Ten typed and eight SQL retained saves, plus eight CLI saves/readbacks, match the
same frozen files. Exact-carrier, stale-handle and overflow controls pass. Historical selected-primary
refusals were superseded by the independently qualified Id update profile. Fixed UPDATE regressions, minimal/journal/preview builds, scoped
strict checks, formatting and dependency boundaries also pass.

See the [write result](../../testing/results/ace16-retained-binary-marker-writes-2026-11-02.json).
These are TableCraft engine/adapter/CLI and independent-reader observations.
Microsoft Access save/reopen, Windows mixed-client locking and general native
save compatibility remain unqualified. The source record remains an earlier
source-only result, not a retroactive save claim.
