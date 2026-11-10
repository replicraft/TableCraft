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
`FirstTableDefinition::capture` retains the original generated profile: the first
field is an Increment Int32 primary key, with protected policies/defaults.
`capture_manual` is a separate request profile with no generated fields. It
accepts ordinary fields with no key, or one explicit Int32, Int64 or Currency
primary at its actual field position. It never inserts an ID, changes a manual
field into AutoNumber or relocates a selected key to the first field. The owned
`primary_name()` and `primary_position()` are optional, and `is_manual()` retains
the original request profile. Description is separate inert metadata and is
permitted on a generated or manual primary too. It retains absence versus present-empty text without trimming
or normalization. The constructive profile conservatively counts at most 255
UTF-16 units, following Microsoft's documented [255-character Description limit](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/description-property-access);
that counting choice is not native supplementary-character parity evidence.
The MR2 property uses declaration flag 1, Text type 10 and exact UTF-16 bytes.
Description does not execute expressions, grant a default or constrain rows.
The selected construction family checks complete property and catalog storage
independently of the per-field limit. Native constructive admission remains authoritative. The
current operation creates an empty table; entered records, broader generated
value handling and other schema operations require their own qualified routes.

## Native declarations and TCNT7

`FirstTableDefinition::capture_native` and `capture_manual_native` retain up to
255 explicit fields in the native construction family. Generated declarations
retain the first Increment Int32 primary; manual declarations retain the actual
optional numeric primary position or no key. `is_native()` identifies this
request language independently of `is_manual()`. Names, exact scalar carriers,
constant defaults, Required/AllowZeroLength and Description retain their checked
semantics. Native declaration validation does not authorize a source or a save.

`NativeImage::plan_first_table_native` still requires the exact canonical blank
BEFORE. It writes the logical definition across linked TDEF pages when needed,
and stores complete MR2 properties in owned external LVAL pages. Every continuation header, logical byte, padding region, map and
property-chain member is independently checked. Existing source bytes outside
the qualified construction scope remain protected. Physical carrier capacity,
checked native file geometry, fallible allocation, cancellation and caller budgets
remain effective; they are not replaced by a per-declaration byte quota.

The original request is `TCNT0007`/profile 7. Its 64-byte header retains the exact
canonical length and complete BEFORE binding. Header flag bit 0 identifies a
manual declaration; all other bits are zero. Manual records use the final u16
word for the actual primary position or `0xffff` for no primary. Generated records
keep both that word and the flags zero. The u16 field count admits 1–255 fields;
field tags and optional property carriers preserve their explicit meanings.
`decode_native` accepts only this exact family. The original `decode` remains
closed to TCNT1–6, and their declaration grammars and output bytes are unchanged.

`FirstTablePlan::prepare_native` uses the V3 resizable journal. Its ordinary
`prepare` refuses a native request before journal or target writes. Native cold
recovery requires `NativeFirstTableIntent::qualify_native_recovery` with the same
`CheckedV3` owner. It verifies the original binding, reconstructs the native plan
in a fresh scope, and compares complete BEFORE and AFTER bytes before permitting
recovery or settled-image confirmation. The V2 qualifier refuses TCNT7; the native
qualifier refuses original request families. No decoder or planner is retried.

The Access adapter selects the constructive family from the retained definition,
encodes its exact intent before journal setup, and uses the corresponding protocol
for apply, recovery and confirmation. Schema receipts omit row counts. Independent
255-field whole-image fixtures and V3 cold-recovery checks are local engineering
evidence; they do not establish Microsoft Access save/reopen compatibility.

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
the retained filesystem journal pair and the selected resizable journal: V2 for
TCNT1–6, V3 for TCNT7.

1. Encode the original schema intent before creating the fresh bundle: `TCNT0001`
   when defaults and policies are absent, `TCNT0002` for checked constant defaults
   without policies, `TCNT0003` when an explicit field policy is present, or
   `TCNT0004` for any default on a newly supported Number/GUID target. An explicit
   Description, including an empty one, selects `TCNT0005` for a generated
   declaration. An explicitly manual/keyless declaration selects `TCNT0006`,
   whether or not it also has Description, policies or defaults. An explicitly
   native definition selects TCNT7, independently of those property choices.
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
The original three profiles retain the same canonical original binding and fresh AFTER
reconstruction. None executes a default expression or generates an initial record.

Wider-default `TCNT0004` uses construction profile 4. It preserves the same
canonical BEFORE, declarations and optional policy bits, while accepting Byte,
Int16, finite Single/Double and exact declared Decimal defaults. An ordinary GUID
can retain an explicit NULL default; non-NULL GUID DefaultValue expressions remain
unqualified. Version 4 is selected when any default, including NULL, targets one
of those newly supported families. Merely declaring a wider field without a
default still uses the older applicable version. Profiles 2 and 3 continue to
reject defaults on those targets, even NULL: their original constructor did not
accept them, so recovery cannot silently enlarge their closed language. Version 4
without a wider-target default refuses.

Description-bearing `TCNT0005` uses construction profile 5 and requires at least
one present Description. It retains the same canonical BEFORE binding, declared
field types and optional policies/defaults as the wider profile. Bit 5 (`0x20`)
marks Description presence; immediately after that field's exact name, a
little-endian u16 UTF-8 byte length and exact description bytes precede any
constant default payload. A zero byte length is present-empty, not absence.
Description is also allowed on the generated primary; its Required,
AllowZeroLength and default restrictions remain unchanged. Invalid UTF-8,
over-limit UTF-16 length, missing Description presence, unknown flags and
version/profile mismatches refuse. Original TCNT1–4 encodings, closed grammars,
record ceilings and reconstruction remain unchanged; they cannot acquire the new
presence bit or a larger record merely because version 5 exists.

## Manual and keyless original intent

`TCNT0006` uses construction profile 6 and the same exact seven-page canonical
BEFORE length and binding. The header remains 64 bytes. Within TCNT1–6, only
this version uses the final u16 header word as a zero-based primary field
position. `0xffff` explicitly means no primary key and requires a zero
primary-name byte length. A keyed record requires a nonempty primary name and a
position within the declared field count. Position zero is a valid manual key;
it is not absence.

V6 accepts only ordinary field tags; generated tag zero is forbidden at every
position. The selected primary must have the native constructive Int32, Int64 or
Currency key family. Decoding restores the key on that actual field and calls
`capture_manual`; it never calls generated-field construction to fill a missing
key. Optional Description, existing typed constants and applicable field policies
retain their exact presence/value semantics. Description is not required in V6.
Native declarations, inline metadata capacity, primary constraints and all normal
resource/cancellation controls remain mandatory.

The V6 decoder checks complete framing, optional-key consistency and scalar/carrier
tags before retaining decoded ownership. Typed declaration validation still
precedes retaining the owned schema. It keeps the
existing bounded described-record ceiling rather than introducing an unbounded
manual schema. Profiles 1–5 preserve their original byte languages, reserved-zero
headers, record ceilings and generated-first-field behavior. They do not gain
manual ordinary fields or keyless admission. Recovery selects the recorded
version once, reconstructs the same manual/keyless plan from canonical BEFORE,
and compares the complete rebuilt AFTER through the existing checked V2 session.
A different key decision, field position or declaration cannot authorize the
recorded replacement.

This first-table route applies only to canonical-blank creation. The separate
[append-table operation](ADD_TABLE_PERSISTENCE.md) now supports qualified mixed
manual/keyless/generated ledgers through TCAT5, with the actual source digest
and freshly checked allocation roles. Its existing TCAT1–4 actions still refuse
nested TCNT6; their legacy/described template grammars and source restrictions
are not widened. Neither operation establishes full Access Design, Microsoft
Access save/reopen or Windows filesystem parity. The verification checkpoints
below retain their recorded scopes; later manual/append results are separate.

These schema-intent changes do not widen the existing TCNA5 clock envelope.
It retains nested TCNA2 only: construction and recoverable planning reject wider
original supplied carriers before native qualification or clock callbacks, and
decode checks the nested version/profile before allocating rows. TCNA6 cannot
be nested in TCNA5. Legacy omitted/NULL original fields remain possible even
when their native target declarations are wider; fresh source qualification still
decides admission. There is no new clock-intent version.

V4 value tags 8/9/10/11/12/13 retain Byte/Int16/Single/Double/GUID/Decimal payloads
respectively. GUID non-NULL payloads are still rejected by declaration validation.
Single and Double payloads preserve exact IEEE bits; Decimal retains its signed
coefficient and scale. Float default text uses bounded scientific notation, with
finite and exact bit-roundtrip checks, including signed zero and subnormals.
Decimal declaration validation requires the exact declared scale and precision;
no default-value rounding or implicit rescaling occurs. Rendered native expressions
retain the existing 255-UTF-16-unit ceiling. These are checked constructive rules,
not native Access parity evidence for every floating-point boundary.

The eight-byte per-field header retains its existing layout. Profile 3 uses its
flags byte as follows (V4, V5 and V6 preserve these policy bits):

| Bit | Meaning |
| --- | --- |
| 0 | A typed constant default payload follows the name |
| 1 | Required is explicitly present |
| 2 | Required is true; valid only when bit 1 is set |
| 3 | AllowZeroLength is explicitly present |
| 4 | AllowZeroLength is true; valid only when bit 3 is set |

A present policy with its value bit clear is explicitly false. Unknown bits,
value bits without presence, any flags on the generated primary in V1–4, AllowZeroLength
on a non-Text field, mismatched version/profile pairs and profile 3 records with
no policies refuse. Profile 4 requires a wider-target default instead of requiring
a policy. Profiles 1 and 2 retain their narrower flag rules. Constant
defaults keep the existing carrier checks and 255-UTF-16-unit rendered-expression
ceiling, including quoting and escaping; policies do not relax that bound.

Required constrains NULL independently of empty Text. A Required field can accept
empty Text when AllowZeroLength is explicitly true; false refuses empty Text.
Stored defaults do not bypass either rule. The later
[normal Increment INSERT](NORMAL_INCREMENTING_AUTONUMBER.md) checks actual native
policies and original input presence before returning a qualified row plan.

The intent retains original declarations rather than physical page positions or
replacement-image authority. Recovery inspects the protocol selected by the exact
request family and reconstructs its original schema plan under the same native
qualification as application.
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
