# Saving the first table in a newly created database

The offline Access adapter exposes `OfflineWriteSession::plan_first_table` and
`plan_first_table_if_original` when both `offline-write-session` and `ace16-create`
are enabled. This is a source-bound schema operation for the exact canonical
system-only blank ACE 16 image. It does not create tables in arbitrary imported
files, alter an existing schema or grant general native Access save compatibility.

The caller supplies an owned `FirstTableDefinition`, retaining its exact table,
field and primary-index names and declarations. Ordinary companions retain exact
optional Required and AllowZeroLength policies: absent, explicitly false and
explicitly true remain distinct. AllowZeroLength applies only to sized Short Text.
The first field is an Increment Int32 primary key; its policies and defaults
remain protected. Native constructive admission remains authoritative. The
current operation creates an empty table; entered records, broader generated
value handling and other schema operations require their own qualified routes.

## Planning and same-file persistence

`first_table_source_available` checks an already captured immutable image using
the same complete-byte canonical predicate as planning. Only a source mismatch
returns false. Construction, cancellation and resource failures propagate. This
read-only availability fact neither grants write authority nor replaces the
standard original capture and exclusive-target checks below.

Planning captures the complete source from the session's retained exclusive target.
The `_if_original` form compares that capture against the complete host-reviewed
bytes before native schema admission. A failed capture retains the owner in
`RefreshRequired`; a completed but refused plan creates no journal and leaves the
same target available for explicit release or another qualified request.

`StagedFirstTable` reports the native table name and field count before durable
effects. It can return its session without applying. Its consuming `apply` uses
the existing retained filesystem journal pair and native V2 resizable journal:

1. Encode the original schema intent before creating the fresh bundle: `TCNT0001`
   when defaults and policies are absent, `TCNT0002` for checked constant defaults
   without policies, or `TCNT0003` when an explicit field policy is present.
2. Prepare the complete checked BEFORE/AFTER transaction against the same target.
3. Commit the native resize/write operation once.
4. Retire the acknowledged journal and follow the actual platform cleanup policy,
   retaining the same target lease.

This path does not publish another fresh file, replace a pathname, regenerate an
arbitrary original database or route through `RequiredOutput`. Namespace setup,
uncertain commit, cleanup and release errors retain the same resources through
`Failure`; no automatic retry, rollback, final-file deletion or unlock follows.
On macOS/Linux, finalization removes the verified retired bundle and acknowledges
the captured-parent barrier. Windows now uses the separate local-NTFS flat
sidecar owner: write-through retirement rename plus retained-file flush, then
strict same-target transfer with the retired artifact intact. It never claims
durable deletion or retries failed removal as retention. Fresh-file publication
and same-file persistence remain distinct protocols.

## Typed completion and recovery

Successful application reports `SchemaChange::FirstTableCreated { column_count }`
through `Receipt::schema_change()`. `matched_rows()` and `changed_rows()` both
return `None`; creating an empty schema is not a zero-row UPDATE. Existing row
operations retain their original matched/changed semantics and no schema change.
`retirement()` reports the actual acknowledged namespace mechanism; `cleanup()`
reports `Removed` on Unix or `RetainedArtifact` for the Windows owner only after
the corresponding completion. Native commitment alone fills neither fact.

Explicit recovery selects the TCNT decoder once from its own action prefix. A
corrupt or unsupported TCNT frame never falls through to an UPDATE, INSERT or
DELETE decoder. Construction profile 1 fixes the exact seven-page canonical blank
and its complete source binding. Both encoding and decoding reject a different
binding; decoding checks it before allocating owned declarations. A future blank
construction cannot silently reuse this profile for new or recovered operations.

Default-bearing construction profile 2 retains exact typed constant values,
including absence versus explicit NULL. Its closed framing validates all fields
before bounded ownership; unsupported carriers, flags, profiles and malformed
payloads refuse. Policy-bearing `TCNT0003` uses construction profile 3. It retains
both optional Boolean policies alongside the same optional typed constant defaults.
It is emitted only when at least one policy is explicitly present, including an
explicit false. Profiles 1 and 2 keep their original wire encoding and reconstructed
native bytes; their records do not acquire inferred policies during recovery.
All three profiles retain the same canonical original binding and fresh AFTER
reconstruction. None executes a default expression or generates an initial record.

The eight-byte per-field header retains its existing layout. Profile 3 uses its
flags byte as follows:

| Bit | Meaning |
| --- | --- |
| 0 | A typed constant default payload follows the name |
| 1 | Required is explicitly present |
| 2 | Required is true; valid only when bit 1 is set |
| 3 | AllowZeroLength is explicitly present |
| 4 | AllowZeroLength is true; valid only when bit 3 is set |

A present policy with its value bit clear is explicitly false. Unknown bits,
value bits without presence, any flags on the generated primary, AllowZeroLength
on a non-Text field, mismatched version/profile pairs and profile 3 records with
no policies refuse. Profiles 1 and 2 retain their narrower flag rules. Constant
defaults keep the existing carrier checks and 255-UTF-16-unit rendered-expression
ceiling, including quoting and escaping; policies do not relax that bound.

Required constrains NULL independently of empty Text. A Required field can accept
empty Text when AllowZeroLength is explicitly true; false refuses empty Text.
Stored defaults do not bypass either rule. The later
[normal Increment INSERT](NORMAL_INCREMENTING_AUTONUMBER.md) checks actual native
policies and original input presence before returning a qualified row plan.

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

The policy follow-up adds complete independently authored native-image
comparisons for all eight nonempty combinations of optional Required and
AllowZeroLength. Independent V3 wire tests retain explicit false, optional
constants, malformed-frame refusals, exact resource admission and cancellation.
Recovery compares the reconstructed complete AFTER against independent native
policy bytes and refuses changed intent policies or replacement metadata without
writing. Original V1 and V2 wire goldens remain passing evidence. These are local
headless checks; native Access comparison and Windows/Linux filesystem execution
remain separate qualification requirements.

This operation provides the persistence component for an unsaved initial table.
It does not itself implement the UI draft, field inference, save-name dialog or
save/discard/cancel workflow. See [database creation](../../ui/DATABASE_CREATION.md)
and [offline native editing](QUERY_EDITING.md).
