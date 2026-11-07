# Saving the first table in a newly created database

The offline Access adapter exposes `OfflineWriteSession::plan_first_table` and
`plan_first_table_if_original` when both `offline-write-session` and `ace16-create`
are enabled. This is a source-bound schema operation for the exact canonical
system-only blank ACE 16 image. It does not create tables in arbitrary imported
files, alter an existing schema or grant general native Access save compatibility.

The caller supplies an owned `FirstTableDefinition`, retaining its exact table,
field and primary-index names and declarations. The first field is an Increment
Int32 primary key. Native constructive admission remains authoritative. The
current operation creates an empty table; entered records, broader generated
value handling and other schema operations require their own qualified routes.

## Planning and same-file persistence

Planning captures the complete source from the session's retained exclusive target.
The `_if_original` form compares that capture against the complete host-reviewed
bytes before native schema admission. A failed capture retains the owner in
`RefreshRequired`; a completed but refused plan creates no journal and leaves the
same target available for explicit release or another qualified request.

`StagedFirstTable` reports the native table name and field count before durable
effects. It can return its session without applying. Its consuming `apply` uses
the existing retained filesystem journal pair and native V2 resizable journal:

1. Encode the original `TCNT0001` schema intent before creating the fresh bundle.
2. Prepare the complete checked BEFORE/AFTER transaction against the same target.
3. Commit the native resize/write operation once.
4. Retire and clean the acknowledged bundle, retaining the same target lease.

This path does not publish another fresh file, replace a pathname, regenerate an
arbitrary original database or route through `RequiredOutput`. Namespace setup,
uncertain commit, cleanup and release errors retain the same resources through
`Failure`; no automatic retry, rollback, final-file deletion or unlock follows.
The filesystem-journal platform limits remain separate from Windows fresh-file
publication support.

## Typed completion and recovery

Successful application reports `SchemaChange::FirstTableCreated { column_count }`
through `Receipt::schema_change()`. `matched_rows()` and `changed_rows()` both
return `None`; creating an empty schema is not a zero-row UPDATE. Existing row
operations retain their original matched/changed semantics and no schema change.

Explicit recovery selects the TCNT decoder once from its own action prefix. A
corrupt or unsupported TCNT frame never falls through to an UPDATE, INSERT or
DELETE decoder. Construction profile 1 fixes the exact seven-page canonical blank
and its complete source binding. Both encoding and decoding reject a different
binding; decoding checks it before allocating owned declarations. A future blank
construction cannot silently reuse this profile for new or recovered operations.

The intent retains original declarations rather than physical page positions or
replacement-image authority. Recovery inspects V2 and reconstructs the original
schema plan from its recorded definition, under the same native qualification as
application.
The schema change is reported only for an acknowledged committed replacement.
A rollback reports the original blank and no schema change.

Intact retired-bundle confirmation uses the existing write-denied storage path.
Its receipt describes the currently verified Original or Replacement image;
it does not invent a historical commit/replay claim. Only Replacement carries
the schema change. Both recovery and confirmation omit row counts, and neither
permits replaying an already known completion.

## Verification boundary

Feature-local tests cover the two initial ID-only and ID-plus-Short-Text schemas,
complete comparison with independently authored raw fixtures, same-file identity,
review mismatch/noncanonical refusal, retained V2 recovery, retired confirmation
and selected-decoder refusal. All five focused adapter tests passed locally on
macOS on 7 November 2026. These include both initial schemas, both recovery
outcomes and both retired current images. They do not establish Windows
filesystem-journal behavior, power-loss durability or native Access save parity.

This operation provides the persistence component for an unsaved initial table.
It does not itself implement the UI draft, field inference, save-name dialog or
save/discard/cancel workflow. See [database creation](../../ui/DATABASE_CREATION.md)
and [offline native editing](QUERY_EDITING.md).
