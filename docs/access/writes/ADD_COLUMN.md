# Saved-table column addition

The normal desktop stages qualified saved-field additions through shared builders,
runtime reviews and native saves. The retained typed route supports Boolean,
Byte, Integer, LONG, Large Number, Single, Double, Currency, legacy DateTime,
GUID, Decimal and Short Text. Existing Number/Short Text request languages remain
separate. General saved type/property alteration and imported ALTER support
remain unfinished; current source, storage and declaration limits follow below.

## Typed retained declarations

`TCAC0012` binds one validated typed declaration and its original name to the
complete BEFORE image. The native candidate captures the actual creation
column, including scalar kind, storage width, Decimal precision/scale, native
Short Text UTF-16 capacity and Unicode compression. The consuming runtime
review reports that captured declaration; a matching request DTO alone grants
no write authority. Checked V3 preparation and fresh recovery reconstruction
remain separate from all older TCAC request languages.

The saved gallery offers Short Text, Number, Large Number, Currency, Date/Time
and Yes/No. Number has Byte, Integer, Long Integer, Single, Double, Decimal and
Replication ID field sizes. Replication ID is ordinary stored GUID data, not
GUID generation or replication. Decimal uses declared precision 1–28 and scale
0–precision, initialized to 18/0. Short Text uses 1–255 native UTF-16 units and an
explicit compression setting. These controls reuse the initial Design declaration
owners; they do not create a phantom table or generated primary field.

All new fields use Required(false). Short Text also uses AllowZeroLength(true).
The current gallery supports the type-default policy, not arbitrary default
expressions, required-field changes or validation authoring:

| Declaration | Stored default for a later omitted INSERT |
| --- | --- |
| Byte, Integer, LONG, Large Number | Exact integer zero |
| Single, Double | Finite zero in the selected IEEE carrier |
| Currency | Exact scaled zero, `0.0000` |
| Decimal | Exact zero at its declared scale |
| Boolean | False |
| GUID, legacy DateTime, Short Text | No default; omitted value is NULL |

Metadata ADD never rewrites existing records or backfills defaults. Historically
absent Boolean values read False; historically absent fields of the other kinds
read NULL. A fresh INSERT separately resolves the stored declarations and defaults.
Explicit NULL remains distinct from omitted input for nullable types; Boolean
accepts only True/False. An absent Boolean bit uses the independently qualified
[TCNI23 materialization service](RETAINED_CELL_UPDATES.md#historically-absent-boolean-bits),
not the present-bit TCNI20 writer. Other subsequent edits likewise require their
own row-source qualification; metadata success grants no row-write capability.

Availability publishes an immutable collection of independently inspected typed
default declarations, each with its measured maximum name width. Missing entries
remain unavailable; Number availability does not imply another scalar kind can
fit. Changing Decimal or Text settings requires a fresh consuming review of that
actual declaration. Source corruption, cancellation and resource exhaustion remain
typed failures rather than unavailable facts or fallback attempts.

TCAC12 uses the retained schema admission and placement below: an actual one-page
TDEF/name frame, eligible native identities and scalar declarations, permitted
literal properties, proven program/dependency absence and an existing fitting
hinted catalog-property LVAL page. Joined-definition DDL, additional page
allocation, arbitrary imported metadata, lookup/unknown properties and required
program execution are not qualified by this route. Older TCAC8–11 declarations,
source languages and preparation remain closed.

## Retained tables and live siblings

The TCAC10/11 retained routes add nullable LONG with Required(false) and literal default
zero, or nullable Short Text with Required(false), AllowZeroLength(true), explicit
width/compression and no default. It uses the current descriptor/property encoders
under a separate retained-schema source family. Existing records are not rewritten
or backfilled: historical absent fields read NULL; later omitted insertion uses
the actual stored default or absence of a default.

Metadata selects retained capture before allocation. The schema source proves
the complete retained cohort, explicit Name AutoCorrect OFF, actual empty
permission/relationship storage, target declarations, dependencies and program
absence. Its schema-purpose certificate cannot substitute for INSERT, UPDATE or
DELETE certificates. Dynamic owned catalog/row/LVAL traversal retains ownership,
cycle, terminal-alias and cross-role overlap checks. Appended tables and original
sparse tables beside live siblings do not inherit the older eleven-page/TD7
geometry restriction. General imported application/schema support remains
unfinished.

Native column-number high water, greatest live stored ID and fixed/variable
placement come from the actual declaration. The new complete property snapshot
is appended into an actually allocated, hinted catalog-property LVAL page with
measured fitting capacity, selecting a fitting candidate from the actual map.
The parser-bound selected catalog terminal receives its Single descriptor;
inactive record padding and old property bodies remain retained. This route
does not allocate pages or rewrite usage hints. Exhausted physical capacity
refuses before saving rather than imposing a new Access size/count quota.

The independent AFTER proof uses the same captured source family, checks the
exact new declaration and property group, and preserves complete DATA/INDEX,
sibling, map, counter and historical bytes outside the proven metadata effects.
The existing qualified ASCII naming and scalar/index/program admission remain
implementation boundaries, not Access restrictions.

`TCAC0010` binds retained Number addition and `TCAC0011` binds retained Short Text
addition. They retain original names, declaration parameters and complete BEFORE
binding under the native file envelope. Exact retained decoding, checked V3
preparation, fresh reconstruction and a separate retained recovery owner protect
both complete images. Older TCAC8/9 keep their original source, V2 preparation and
decoding contracts; failures never retry another family. Journal-only builds can
recognize retained request data but refuse native reconstruction without the
creation feature. Runtime/Access own capture, review and durable sessions;
desktop composes those services through the existing field-addition workflow.

The earlier LONG/Short Text macOS ARM64 verification passes 112 retained native tests (twelve existing
ignored cases remain unqualified), fourteen native ADD tests and twelve
field-addition intent/recovery tests. The latter retain exact forged-image and
wrong-journal refusals. Three journal-only tests execute data decoding and the
disabled-writer refusal. Three independent Access read/query cases and two
Access staging/recovery cases pass. All 42 headless GUI field-addition tests
pass, including six retained ADD/fill/omitted-INSERT/NULL-INSERT sequences with
fresh reopening and independent whole-file comparisons; all 38 table-creation
regressions pass. These observations do not establish Windows/Access parity or
general imported ALTER support.

## Addition after a populated field deletion

The current-schema route distinguishes the active descriptor count from retained
native column numbers, stored IDs and variable-slot counts. A qualified Number
or Short Text addition after tail-field deletion appends a new logical identity;
old rows therefore read the new field as NULL, without exposing retired payloads.
Metadata ADD preserves DATA/index bytes and the original property dictionary.
The original creation-history request languages retain their existing scope.

The allocation rules follow the separately observed current metadata: native
number uses the retained maximum; stored ID follows the greatest live ID; fixed
offset follows surviving fixed fields; variable ordinal follows surviving
variable fields. Physical tail slots may therefore be reused, while native
logical identities remain distinct. These independently implemented rules are
informed by [Jackcess TableUpdater](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/TableUpdater.java)
and [TableImpl](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/TableImpl.java);
they are not evidence of a native Access producer comparison.

`plan_recoverable_current_row_updates` binds a separate `TCNI0016` original
request to the whole BEFORE image. Its current implementation accepts one
unindexed ordinary LONG or Short Text cell on a qualified current-schema source,
including direct and forwarded records. Original expected carriers are checked
before serialization. The row encoder and separate reconstruction proof retain
retired fixed bytes, unrelated variable payloads, original padding and mask bits;
newly materialized records write the retained variable-footer count and use the
selected field's actual native number. Empty Text, NULL and supplied values stay
distinct. This implementation scope is not an Access row or batch limit.

The Access adapter selects this route from original metadata before planning.
It never retries a failed historical operation under another language. Runtime
and desktop use their existing consuming review/save/reopen services. Decoding
`TCNI0016` grants no write authority: cold recovery reconstructs fresh source,
program, constraint, ownership, placement and complete-image evidence. Original
`TCNI0011`–`TCNI0015` request meanings remain unchanged.

The focused native row batch passes six tests, including fourteen source/target
combinations, original wire bytes, fresh-scope reconstruction, pending/committed
cold recovery and forged AFTER refusal. Eleven metadata tests and the retained
framing/source-purpose tests pass separately. Independent file reads verify
first fills and repeated Text replacement. These observations do not establish
native Windows/Access Save/reopen compatibility.

Fourteen headless desktop journeys perform real DROP → ADD → fill → reopen
through the shared runtime, with forty-four reviewed cell saves and independent
complete-image checks. Surviving fields, NULL/empty/Unicode distinctions, scratch
SQL and object order remain intact. DROP/ADD schema publication preserves tab
identities under its source witness; ordinary cell publication issues fresh IDs
and rejects stale targets under the existing source-free layout contract.

## Current Number addition over populated records

`prepare_current_number_field_addition` and its consuming
`plan_recoverable_current_number_field_addition` now implement a separate ordinary
nullable Long Integer addition with stored Required(false) and DefaultValue zero.
The source is positively qualified current metadata in the restricted unencoded,
eleven-page ACE2016/subversion-zero layout with explicit Name AutoCorrect OFF.
Actual rows, indexes, live property ownership and protected history are checked;
remembered creator state does not grant permission. Existing table/field names
and the requested field must be ASCII. Broader layouts, Unicode collision
semantics, other declarations and Required(true) remain separate work.

Existing records retain their exact bytes and expose the absent new field as
NULL. A later omitted insertion uses zero, while explicit NULL remains NULL.
Independent complete-image oracles and connected desktop insertion/reopen checks
cover those distinctions. The separate operation below can fill one absent
Number field in qualified fixed or mixed records; the ADD itself still
does not rewrite old records or qualify arbitrary record expansion.

### Editing an existing field

Existing unindexed Int32 fields in those historical records now have a separate
normal UPDATE route. `plan_historical_present_fixed_updates` requires its own
current-source proof and mutation-purpose no-required-program certificate, then
checks the target against each original record's stored field count. It retains
the original count, mask geometry, ignored payload, absent later fields, all
indexes and metadata. Full scalar constraints, relationships, property/code
closure and the complete independent fixed-record proof still apply. Generated
or indexed fields and absent targets refuse, including absent NULL-to-NULL
requests; no record is expanded by this route.

The consuming `plan_recoverable_historical_present_fixed_updates` binds only
original NULL/Int32 cells in the distinct `TCNI0011`/profile11 language. Recovery
repeats that specific source proof and reconstructs both complete images before
the existing checked V1 journal can recover. Old ordinary intent languages do
not inherit historical admission. The Access adapter selects this source from
original layout/record metadata before attempting a proof; a refused selected
proof never retries ordinary editing.

Two headless desktop journeys pass through actual ADD widgets, replacement/NULL
inputs, review, consuming Save and guarded fresh reopen. Independent complete
images confirm `Score` value-to-value, NULL-to-value and value-to-NULL edits while
every absent `Bonus` remains NULL, with no remaining journal. An explicitly
issued absent NULL-to-NULL review refuses without changing the original. These are constructed
ACE16 source outcomes, not imported-database or native Access qualification.

### Filling an absent Number field

`plan_recoverable_historical_number_materialization` implements one absent
nullable, unindexed LONG field's NULL-to-Int32 change in the same closed
eleven-page source, with dense all-LONG declarations and no variable tails.
It shares the positive current-source/program/constraint proof with existing-field
editing, then separately qualifies the actual original stored count and one-cell
request. Bulk requests, indexed/generated fields, NULL-to-NULL materialization,
non-Int32 input, variable layouts, exhausted physical slots and unavailable owned
space refuse. This is neither EOF allocation nor a full-row insertion/default
evaluation path.

The new serializer copies original fixed payloads, including ignored NULL bytes,
and rebuilds the declared fixed extent. Other newly represented fields remain
NULL; stored defaults are not applied. Unused mask bits remain opaque unless
the new count makes them meaningful. Independent reconstruction verifies the
complete record before existing owned-space reservations and forwarding append
one hidden target. The original logical root becomes a forwarding pointer within
its audited edit scope; ID index references and logical row counts stay unchanged.
The independent placement proof checks every other byte and ownership role.

`TCNI0012`/profile12 retains only the original request and complete BEFORE binding,
never a physical target or pointer. Fresh review and cold recovery reconstruct
the exact full AFTER before using the checked V1 journal. The shared normal
selector inspects actual original field presence before choosing profile11 or12;
neither planner is retried after the other fails. Both data records remain
decodable without the optional writer capability, but reconstruction then refuses
with `UnsupportedProfile`.

The connected headless journey adds `Bonus`, saves values in all three old rows,
edits `Score` through the resulting forwarded root and opens a fresh desktop host.
Every step matches a complete independently authored image and leaves no journal.
Native tests separately cover another absent field staying NULL, opaque payload
and mask bits, slot exhaustion, original-intent bytes, crossed profiles,
pending/committed cold recovery and forged-journal metadata refusal. These are
implementation checks, not Microsoft Access or native Windows save qualification.

### Filling an absent Number beside Text or Binary

The separate `TCNI0013`/profile13 operation extends that workflow to the same
qualified current source with known fixed fields and Short Text or Short Binary
variable slots. It still accepts exactly one absent nullable, unindexed LONG
field changing from NULL to Int32 through a direct logical root. Profile12's
all-LONG source language remains unchanged; selection uses original metadata,
without retrying a rejected planner as another profile.

Record reconstruction preserves the entire original variable body, including
compressed text, opaque NULL payloads and stationary padding. It adjusts the
offset directory for the larger fixed prefix and represents other absent fields
as NULL rather than applying defaults. Independent record and placement proofs
check the resulting complete image before the existing owned-space forwarding
operation is admitted. Recovery stores the original request and full BEFORE
binding, then reconstructs the AFTER under fresh source and resource checks.

Headless desktop journeys add the Number field beside Text or Binary, fill each
of three original rows, edit an existing field through the resulting forwarded
root and reopen a fresh host. Every stage matches an independently authored
whole image. Forty-four focused native historical tests cover the combined
profiles, including independent wire bytes, crossed-profile rejection, cold
pending/committed recovery and forged offset-directory rejection. The connected
field-addition batch passes fifteen cases. These observations do not qualify
arbitrary imported layouts or bulk changes. Repeated growth through a forwarded
root uses the separate operation below.

### Filling another Number through a forwarded root

The separate `TCNI0014`/profile14 operation accepts one absent Number field through
an already-forwarded logical root in the same qualified fixed or mixed source.
The original request and complete BEFORE binding are retained; source, record
serialization and placement proofs are reconstructed during review and recovery.
The previous hidden terminal remains byte-for-byte intact. A new hidden terminal
is appended within owned capacity, and only the original logical root's pointer
is retargeted. Index references, logical row counts, other values and absent-field
NULL semantics remain unchanged.

Normal historical UPDATE selection checks actual missing-field and root metadata
before choosing the operation. Profile14 refuses direct roots and present fields;
profiles12/13 retain their direct-root requirement. No refused planner is retried
as another profile. Five focused native tests pass across fixed, Text, Binary and
later-absent-Text sources. Independent complete images, exact request bytes,
fresh-scope review, crossed-language refusals, pending/committed cold recovery
and a forged journal AFTER are checked. The data-only journal feature also
decodes profile14 but refuses native reconstruction without the writer feature.
The connected two-add desktop journey and native Access comparison remain
separate verification work.

The closed `TCAC0008`/profile8 request binds the complete original and exact names,
fixing this Number declaration. Recovery reconstructs both complete images under
separate native/proof budgets before using the checked V2 journal. TCAC1–7 retain
their source languages and are never retried after a failed decoder or proof.
Four focused intent tests pass: independently authored wire bytes, fresh-scope
requalification, malformed borrowed-name/header refusal before allocation,
pending/committed cold recovery and forged AFTER rejection. A synthetic Unicode
request also confirms that successful decoding grants no native write authority.
These tests do not establish Windows or Microsoft Access Save/reopen behavior.

## Current Short Text addition and row editing

`prepare_current_short_text_field_addition` and consuming
`plan_recoverable_current_short_text_field_addition` add nullable Short Text to
populated tables in the current-metadata ACE16 source above. The declaration
stores Required(false), AllowZeroLength(true), explicit capacity of 1–255 UTF-16
units and explicit Unicode compression permission, with no DefaultValue. Existing
records are preserved byte for byte and expose the new field as NULL. The normal
saved-table gallery chooses capacity 255 and compression enabled. This follows
Microsoft's [Short Text field workflow and property table](https://support.microsoft.com/en-us/access/create-or-delete-a-short-text-field);
the field's configured capacity remains distinct from display formatting.

The separate `TCAC0009`/profile9 binds the original complete image, names, width
and compression. Number's TCAC0008 decoder remains Number-only; the explicitly
selected Short Text decoder accepts only TCAC0009. Both use the same current
candidate, source-binding, exact-image comparison and checked V2 journal owners.
Neither a decoded request nor a generic journal AFTER authorizes native recovery.

The `TCNI0015`/profile15 operation updates one unindexed Short Text field through
a direct or already-forwarded historical root in existing owned DATA capacity.
`plan_recoverable_historical_short_text_updates` binds the actual original
NULL/Text carrier before selecting the present-field or absent-field serializer
from the stored record count. A refused branch is never retried. Replacement,
growth, shrinkage, empty text, clearing to NULL and refilling are supported within
that source. Equal values preserve the complete original image, including ignored
bytes beneath NULL. Final row constraints are revalidated before a plan is issued.

The serializer independently verifies historical field counts, variable offsets,
NULL masks, unchanged sibling payloads and padding, and complete physical
placement. Present fields retain the original stored field count and variable
slots; an absent field expands only its selected record. Recovery reconstructs
the same operation from the original source and expected/replacement carriers.
Earlier ordinary and fixed historical request languages retain their proofs.
The shared row capability can admit an ID-only table with a supported Text field;
it does not require an unrelated editable Number field.

The shared Access editor connects this operation to the existing runtime review,
consuming Save and guarded reopen; desktop adds no native editing logic. The full
headless field-addition family passes 39 cases. Four complete widget journeys
cover direct, mixed, forwarded and ID-only tables, each adding the field, filling,
replacing, shrinking, clearing and refilling before fresh reopen. Independent
complete database images verify every save. A separate New-row journey verifies
omitted/explicit NULL, empty and Unicode Text, generated IDs and preservation of
old rows. Native original-request tests cover exact wire bytes, fresh-scope
requalification, both cold journal states, stale expected values and forged AFTERs.

These are constructed source checks, not Windows or native Access Save/reopen
qualification. Broader source layouts, bulk requests, indexed Text targets and
EOF allocation remain separate implementation work. These restrictions are
current writer limitations, not Access format limits.

## Populated-table behavior references

Earlier constructive ADD families still require an empty table. The separate
current-metadata operation above implements part of the following contract;
broader historical-record UPDATE and source cases stay open.

Microsoft documents adding Number fields through both Datasheet's Click to Add
and Design's first blank Field Name/Data Type row, followed by Save. Its field
reference defines Long Integer as a four-byte integer and describes DefaultValue
as applying when a new record is created. See
[Create or delete a Number or Currency field](https://support.microsoft.com/en-us/access/create-or-delete-a-number-or-currency-field)
and [DefaultValue](https://support.microsoft.com/en-us/access/defaultvalue-property).
The latter explicitly distinguishes future-record defaults from existing records;
it does not establish a physical ADD encoding or an Access16 GUI save observation.

The historical-row behavior is **NULL, not a zero backfill**, even when
the new field stores DefaultValue zero. This is an inference from Microsoft's
default contract and the independent Jackcess implementation:
[`TableImpl.mutateAddColumn`](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/TableImpl.java)
changes ordinary field definitions/properties without rewriting existing records,
and [`NullMask.isNull`](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/NullMask.java)
treats a new column beyond an old record's stored column count as NULL. The
retained local Jackcess source agrees for ordinary Number fields. This evidence
does not yet verify that exact Click to Add/Design sequence in native Access16;
Windows comparison remains deferred.

Acceptance distinguishes old records from later inserts: existing
records expose the added field as NULL; a later omitted value resolves to the
stored zero default; explicit NULL remains NULL; and an unrelated edit to an old
record preserves its added-field NULL. Complete independent images, actual
record decoding, journal recovery and runtime/GUI witness reopening must qualify
each operation without inventing a data backfill.

Keep Required(false) explicit initially. Microsoft offers an independent check of
existing records when enabling Required, while enforcing the property for new
records regardless of that choice. Required(true) needs its own validation and
confirmation contract; it must not silently turn historical NULL into zero. See
[Require a value in a field](https://support.microsoft.com/en-us/access/require-a-value-in-a-field).

## Typed Number and Short Text candidate

The separate `prepare_created_add_field_candidate` API now constructs a typed
Number or Short Text candidate from a positively reconstructed empty ACE16
source. Short Text explicitly selects 1–255 UTF-16 units and Unicode compression
permission, Required(false), AllowZeroLength(true), and no stored default. Existing
creation defaults and TCAC1–4 source/request languages remain unchanged.

The proof reconstructs a canonical generated-primary seed and every appended
field/property-history stage. Dictionary order derives from that history; an
arbitrary observed permutation cannot authorize itself. Original field widths,
compression, variable ordinals, index bytes, property rows and the full image
complement remain protected. Only the qualified one-EOF property rollover is
available. The candidate alone supplies no persistence authority or GUI Text
activation; the separate original request below owns native persistence. Legacy Number
purposes remain distinct.

The [candidate workflow batch](../../../tests/results/schema/typed-field-candidate-workflows-2026-11-08.json)
passes 336 unique selected tests, including ten new candidate cases covering
18 independent SDK AFTER images. Three existing ignored cases are excluded.
Strict engine/testkit all-feature lint, creation+journal lint and creation-only
lint pass. Initial feature/lint failures remain recorded separately. Native
Microsoft Access and Windows execution remain unqualified.

## Typed original request and constructive prefix

`plan_recoverable_created_field_addition` seals the actual typed candidate as
`NativeFieldAdditionIntent` and `RecoverableFieldAdditionPlan`. Its captured
`FieldAdditionTarget` is either nullable LONG with stored Required(false) and
Int32 zero, or sized Short Text with stored Required(false), AllowZeroLength(true),
absent DefaultValue and explicit compression permission. Width is 1–255 UTF-16
units; a requested width or compression choice cannot be replaced by a host
inference. The target is obtained from the actual sealed native plan.

The initial source prefix and historical appended suffix have different roles.
The prefix retains the constructive owner's known scalar declarations, constant
defaults, Required/AllowZeroLength policies and inert descriptions. A prefix can
therefore contain a Byte, an Int32 with no default, Required(true), Text with
AllowZeroLength(false), or a typed nonzero constant without converting those
fields into the newly appended declaration. Only historical appended fields
must match the closed Number or Short Text target tuples. The generated LONG
Increment primary keeps its existing policy restrictions. Populated sources,
unknown properties/programs, additional objects and broader histories still
refuse; no required program is skipped.

For n current fields and r ordinary property rows in the actual owned-page ledger,
the proof constructs the n-r field seed from those checked prefix declarations.
Its dictionary order derives from the creator policy. Each historical target
then extends that order only by newly required known keys. Both the derived final
order and the entire resulting source image must equal the original. Existing
Required(true), empty-value rules, defaults and descriptions remain byte-exact;
an observed dictionary permutation cannot serve as its own expected source.

`created_field_addition_source_purpose` is an engine-owned inert observation.
For Number, it retains the original first/repeated Number purpose when the actual
field policies belong to those languages. Native Text compression, any
AllowZeroLength property, Required(true), present Description, or a constant
default other than typed Int32 zero selects the new typed purpose. Short Text
requests select the typed purpose. Header/capture checks precede property
inspection. Exactly one constructor then performs full admission; a failed proof
is never retried as another profile. This observation is not used to reinterpret
old retained requests.

Typed-source qualification reuses complete existing proofs when the observed
source belongs to a legacy Number language. A first Number source consumes the
existing canonical source proof and exact first-Number property policy. A repeated
Number source consumes the actual retained repeated-source proof, including its
checked schema, declarations, full historical ledger and proven initial dictionary
policy. The typed owner still rechecks its generated-field restrictions and derives
known dictionary order from that proved construction. A mixed Text source keeps
its separate canonical-prefix/history replay. There is no second reconstruction
or failed-proof fallback, and no availability Boolean supplies native authority.

Runtime source availability now consumes that one complete engine proof for both
closed field kinds. This replaces duplicate legacy-plus-typed history replay that
exhausted the unchanged normal step allowance on a retained N13 source. Native,
proof and persistence budgets are unchanged; no reset, larger allowance or hidden
retry resolves the failure. Actual review still binds the selected original
purpose and exact target, and performs complete candidate qualification.

The closed `TCAC0005`/profile5 header is 64 bytes: magic, total length, profile,
target tag, Text width, compression choice, complete original length/digest, and
the two UTF-8 name lengths. Number fixes width/compression to zero; Text permits
only width 1–255 and compression 0|1. The target tag fixes the complete policy tuple.
No arbitrary property bytes, ROW selector or AFTER appears in the request.
Unknown versions, combinations, framing and malformed names refuse before owned
name allocation. Decode returns data only.

Fresh normal requalification verifies full original bytes and binding, reconstructs
the entire native candidate, then compares both complete images under separate
native/proof pools. Native recovery spends one cumulative allowance, verifies the
original binding first and reconstructs the same candidate before comparing both
journal images. A valid width/compression/target substitution cannot qualify an
unrelated AFTER. The existing checked resizable V2 session handles initial growth,
fitted images and the qualified one-page property rollover; semantic request
version5 is independent of journal format2. Settled confirmation cannot repair.
TCAC0001 through TCAC0004 retain their own target/source constructors and wire
languages. Shared storage/proof helpers do not widen those languages.

The [coordinated workflow batch](../../../tests/results/schema/field-addition-gui-workflows-2026-11-08.json)
passes 1,505 unique selected checks, including native intent/source/recovery,
adapter, runtime and the complete shared UI/desktop suites. Five existing ignored
cases are excluded. It covers ordinary Text → Number → Text saves, independent
complete-image oracles, subsequent row input, and near-capacity Number inspection
and rollover under the unchanged work allowance. Strict all-feature and smaller
creation-configuration lint pass. The earlier 336-case candidate record above
retains its historical scope. Neither batch establishes Microsoft Access,
Windows or older-format Save compatibility.

## Number and Short Text after explicit OFF

A separate `prepare_created_add_field_after_off_candidate` consumes the retained
`FieldHistoryOffSource`: the complete nine-role explicit-OFF source must first
be reconstructed through its real prior ADD history and OFF insertion. Merely
observing two zero options or dropping the database role from a view supplies no
authority. This profile is currently an empty, unencoded, fitting eleven-page
ACE16 source with one generated-primary user table. Requested and compared table
and field identifiers must be ASCII; native Unicode collision semantics remain
unqualified. Existing TCAC1–5 comparison/source languages are unchanged.

The new target is either NumberLongZero or TEXT(255) with compression enabled,
Required(false), AllowZeroLength(true) and no default. It reuses the actual typed
native declaration, property encoder and selected-origin placement owners. The
user's descriptor selects its live body independently of the database ROW. The
new user body appends after that database ROW without counting the DB body as an
ADD event. Every retired row, the unchanged DB record/body/reference, maps/index
bytes and all complement bytes remain protected. No allocation/rollover or later
event source admission is added by this profile.

`plan_recoverable_created_field_addition_after_off` seals that actual candidate as
`RecoverableFieldAdditionAfterOffPlan` and `NativeFieldAdditionAfterOffIntent`.
Its distinct TCAC0006/profile6 frame binds the complete original length/digest,
exact table/new-field spelling and closed Number or Text255/compressed target.
Width/compression combinations outside those tuples refuse before owned names.
No arbitrary property/ROW/AFTER appears in the request. Fresh requalification
rebuilds the complete owned proof and candidate, then compares both full images
under separate native/proof pools. Recovery reconstructs from the original
request under one cumulative pool, retaining the actual CheckedV2 session.
These fitted candidates are equal length: truncated or grown targets receive no
new repair permission. Semantic version6 is distinct from physical journal V2.

The normal `field_addition_source_purpose` uses the database-properties owner's
actual native containment role, then chooses exactly one complete old or OFF
proof. Old purpose/check APIs and retained requests are unchanged. OFF source
inspection measures the genuine encoder's non-name bytes against the actual
allocator capacity, yielding per-kind ASCII name-unit limits. A shorter valid
name may remain available when 64 units cannot fit. Named review still checks
its exact complete proposal; no guessed default field name or AFTER is built by
inspection. The generic runtime/adapter/GUI field workflow retains its existing
consuming review, schema-only receipt and full-image witness publication.

The [four SDK observations](../../../tests/access/schema/field_add_after_off/README.md) and
independent raw fixtures anchor complete bytes. The
[service record](../../../tests/results/schema/field-schema-services-2026-11-09.json)
reports two separate passing batches: 1,290 recovery/adapter/presentation checks
and 693 source/runtime/host checks. Their repeated coverage is not summed.
Four actual headless widget cases save Number/Text after the two OFF histories,
checking complete reopened bytes, tab identity and no retained journal. That
historical batch retained the later-event refusal; the separate route below now
qualifies subsequent fitted additions. Microsoft Access/Windows and
older-format native Save remain unverified.

## Repeated fitted additions after OFF

`AfterExplicitOffHistory` is a separate metadata-selected purpose. Actual user and
database Single references on their shared property page select its chronology;
row count or a failed old proof never selects this route. Qualification rebuilds
the settled OFF source, replays every later append through the real native kernel
and compares the entire result to the current original image. The private replay
owner then permits the next append. Current names alone supply no authority.

The distinct `TCAC0007`/profile7 request carries only the original binding and
closed target declaration. Cold recovery reconstructs both full images through
that same chronology and checked V2 owner. Old TCAC1–6 languages stay unchanged.
Readonly inspection measures the actual qualified encoder capacity; one normal
runtime review, save worker and complete-image witness serve the GUI. There is
no post-failure retry into another source language or unchecked AFTER payload.

The [independent second-event SDK record](../../../tests/access/schema/field_add_repeat_after_off/README.md)
anchors four complete second additions. Headless runtime and actual GUI cases
match those bytes; a further raw-only history case is explicitly separate from
SDK save evidence. The [coordinated workbench record](../../../tests/results/schema/field-workbench-workflows-2026-11-09.json)
contains passing creation, original-intent recovery, adapter, runtime and GUI
checks. Empty canonical ACE16, ASCII identifiers, existing-page capacity and
sixteen-field source limits remain. This adds no rollover, populated-table or
older-profile write permission and no native Access save qualification.

## Actual source and preservation

`NativeImage::prepare_add_int32_column_candidate` requires a complete standard
capture of the canonical single-table ACE 16 (`Ace2016`) creator layout: one
empty keyless table with one through fifteen ordinary Int32 fields. Actual
decoded table/field spellings select the comparison; no literal source hash,
remembered creator flag or particular name grants admission. Complete source
comparison refuses other objects, properties, nonzero timestamps, indexes,
populated tables, inactive history and unknown bytes.

The operation validates the native proposed name, duplicates and the resulting
sixteen-field ceiling through the existing constructive declaration owner. It
changes only the selected definition page's counts, length/free-space and
appended descriptor/name-vector scope. Other catalog, allocation and user data
bytes remain exact. Recapture checks the resulting declaration geometry. The
independent testkit fixture models the SDK-observed whole AFTER without calling
production codecs; broader names and field-count boundaries remain explicit.

This first operation is not general ADD COLUMN: generated keys, required/default
programs, other scalar types, populated storage changes and broader Jet/ACE
profiles remain work to do. Existing draft authoring remains a separate workflow.

## Original request and journal

With both `journal` and `ace16-create`,
`plan_recoverable_add_int32_column` captures a sealed candidate and
`NativeAddColumnIntent`. The closed `TCAC0001`/profile-1 language has a 60-byte
header, full original length/digest and exact UTF-8 table/new-field spellings.
Profile 1 always means this specific nullable Int32 operation. Unknown versions,
profiles, flags, framing and malformed borrowed names refuse before either name
is allocated. Decoding request data does not grant save authority.

Fresh requalification compares the complete retained original, verifies its
binding, repeats native source/declaration qualification and compares the entire
rebuilt AFTER. Native and proof budgets remain separate cumulative pools. The
consuming plan prepares the existing fixed-length checked V1 image journal;
request version and journal version are independent. Hosts still own exclusive
resources, durable namespace setup, cleanup and recovery discovery.

`qualify_native_add_column_recovery` reconstructs both complete images before
returning the same checked journal session. Explicit recovery restores BEFORE or
retains the committed AFTER according to that session's checked state. Settled
confirmation performs no repair writes. No stored patch, ordinal or arbitrary
AFTER is accepted as mutation authority. Existing row, creation and table-rename
intent languages remain unchanged.

The focused authored tests cover independent request bytes, complete-image
reconstruction, malformed framing, stale originals, request substitution,
pending/committed/torn journals, settled confirmation and independent proof
limits. Five candidate, six native intent/recovery and five offline adapter tests pass.
The adapter stages a reviewed plan against fresh retained-target bytes and
dispatches only the closed TCAC family during cold and settled recovery. A
committed `SchemaChange::ColumnAdded` has no matched/changed row counts.

See the [focused batch record](../../../tests/results/schema/schema-save-workflows-2026-11-08.json).
Native Windows and Microsoft Access save/reopen behavior, timestamp parity and
GUI column editing are not qualified by this checkpoint. Five additional runtime
checks now pass: consuming review/save, full-schema reopen, stale revisions and
capabilities, source drift and whole-content reopen witness validation. The native
and unsupported host services share the typed contract; this gate executes on
macOS only.

## Generated-primary source and versioned request

The separate `prepare_created_add_int32_column_candidate` source proof now passes
seven focused cases, including complete SDK AFTER images for an inline existing
default zero and mixed scalar declarations. It admits canonical empty header-5
creator tables with a first LONG Increment primary and constructively known
inline field properties. Original keys, defaults, descriptions, field metadata
and all bytes outside the definition-page envelope remain unchanged. It still
appends only a nullable Int32 with an absent new default; authoring a new default
zero needs a separate property-storage/allocation operation.

`plan_recoverable_created_add_int32_column` now seals that separate purpose as
`TCAC0002`/profile 2. The old API and `TCAC0001`/profile 1 keep their exact keyless
admission and original wire body. Both use a 60-byte request header and the
existing fixed-length checked V1 image journal. Version/profile pairs are closed;
crossed or future pairs refuse before name allocation. Fresh reconstruction
selects only the captured native constructor, never another profile after a
failure.

The shared runtime selects one declared constructor from actual native index
metadata. Selection does not grant admission: the selected source proof remains
complete, and unsupported layouts refuse. This metadata selection is not used
to reinterpret retained requests during recovery. Three new `TCAC0002`
wire/recovery cases and the expanded runtime save/reopen case pass in the
coordinated local batch. This is not general populated-table alteration or GUI
default authoring.

## Independent generated-primary SDK observations

The [source/provenance record](../../../tests/results/schema/add-column-created-sdk-source-2026-11-08.json)
separates whole-image SDK observations from native candidate and normal-save
qualification. The existing inline-default-zero source and the fourteen-field
Mixed source both yield the complete independently authored AFTER when a nullable
LONG with an absent new default is appended. Only page 7 changes; file length,
primary root, allocation, catalog properties and zero native dates remain exact.
The raw fixture owns original bytes independently of the production serializer.

Explicitly declaring default zero on the new field is a different SDK operation:
it changes catalog and allocation pages and grows the file by one property LVAL
page. It is outside both existing no-default candidate target declarations. The
separate default-zero candidate below passes its focused byte-preservation checks;
normal persistence still needs an explicit original request and matching
resizable journal/recovery qualification. Existing
default-zero preservation does not authorize inventing a new field default or
enabling the ordinary GUI Number-field workflow. All observed SDK dates are
`1899-12-30T00:00`; this does not qualify native Access timestamp changes.

## Separate explicit-default-zero candidate checkpoint

The authored `prepare_created_add_default_zero_int32_column_candidate` is a
separate candidate-only operation, with its own opaque candidate/error types.
The native candidate slice is connected. All six focused Rust cases pass in the
coordinated batch, including both full SDK AFTER-image pins. Seven existing
created-source cases, five keyless candidate cases and nine frozen-intent cases
also pass. The candidate itself supplies no original intent or save authority.
The separate Number route below now supplies native intent, journal/recovery
and runtime services; GUI controls remain disabled. Existing `TCAC0001`/`TCAC0002` admission and
wire meanings remain unchanged.

Its authored source proof requires the complete canonical empty header-5
creator image, a first LONG Increment primary, one through fifteen source fields
and zero native dates. Properties must be absent or consist only of constant
zero defaults and explicit Required false. Existing external property storage,
other dictionaries, extra objects, populated rows, unknown history and nonzero
dates refuse. The complete
target MR2 body must prefer one external LVAL row (65 through 4,076 bytes);
shorter inline and chained targets remain outside this checkpoint.

The [new SDK record](../../../tests/results/schema/add-column-default-zero-sdk-source-2026-11-08.json)
qualifies two distinct independent observations. Adding `Score` with explicit
DefaultValue `0` to `Table1(ID)` grows its 73-byte catalog root into an 85-byte
hidden target, retaining logical root slot 7 through a forwarding pointer to
hidden slot 8. Its logical catalog row count remains unchanged. Adding `Extra`
with the same default to `Items(Id, Score DefaultValue0)` instead keeps the
151-byte physical record, replaces its inline property descriptor and zeroes the
reclaimed inline body. Both grow from 40,960 to 45,056 bytes, change existing
pages 3, 4 and 7, and append LVAL page 10. Only the selected LvProp owned/candidate
map cells gain page-10 membership; the global free map remains exact.

The candidate composes existing declaration/MR2 encoding, variable-record
replacement, represented allocation and audited stable-root placement; it does
not introduce an ordinary system-catalog UPDATE permission. Independent raw
fixtures retain literal record/map/LVAL construction without production codecs.
The separately authored Python literal AFTER reconstruction matches both entire
SDK AFTER images byte for byte. The six Rust cases cover those full image pins,
old no-default behavior and refusal/cancellation/resource boundaries, but have
not been compiled or executed at this checkpoint. No native Microsoft Access
comparison or timestamp/event parity is inferred from the SDK observations.

### Reproduce the independent SDK operations

The maintained [first-column helper](../../../tests/access/schema/fields/CreatedFirstDefaultColumnOracle.java)
is the executed original first-party probe source, promoted unchanged; the
[existing-inline helper](../../../tests/access/schema/fields/CreatedTableColumnOracle.java) keeps its
frozen recipe. Both use the reviewed Jackcess 5.0.4 JAR, Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
See [fixture provenance](../../../tests/reports/reading/FIXTURES.md#explicit-default-zero-saved-column-add-reference--8-november-2026).

Set `JACKCESS_JAR` to that reviewed artifact, `ORACLE_CLASSES` to a local generated
class directory, and `ID_BEFORE`/`INLINE_BEFORE` to the hash-pinned original inputs
in the SDK record. The independently authored sources are
`canonical_created_default_zero::source(Source::IdOnly)` and
`source(Source::ExistingZero)` in the testkit fixture owner. Set `ID_AFTER` and
`INLINE_AFTER` to fresh output filenames that do not exist. Run from the repository
root in a POSIX shell:

```sh
mkdir -p "$ORACLE_CLASSES"
javac -cp "$JACKCESS_JAR" -d "$ORACLE_CLASSES" \
  docs/testing/CreatedFirstDefaultColumnOracle.java \
  docs/testing/CreatedTableColumnOracle.java
java -cp "$JACKCESS_JAR:$ORACLE_CLASSES" CreatedFirstDefaultColumnOracle \
  "$ID_BEFORE" "$ID_AFTER" Table1 Score --default-zero
java -cp "$JACKCESS_JAR:$ORACLE_CLASSES" CreatedTableColumnOracle \
  "$INLINE_BEFORE" "$INLINE_AFTER" Items Extra --default-zero
```

Each helper copies its original, disables expression evaluation, calls public
SDK ADD, reopens read-only, prints complete BEFORE/AFTER SHA-256 and checks that
the original remains byte exact. Database, JAR and generated class bytes remain
local and untracked. These commands replay SDK evidence; they do not execute the
new native candidate or establish normal save/recovery qualification.

## Existing-table UI boundary

Saved native datasheets connect the exact Number declaration through shared UI
naming, runtime review and the dedicated desktop save owner. Host support and a
positive name-independent native source inspection are both required. The older
absent-default primitive remains distinct from the [Number gallery's typed-zero
declaration](../../ui/DATABASE_CREATION.md#adding-a-column-to-an-existing-saved-table).
Painting a provisional zero cannot substitute for persisted default metadata.

The explicit-zero target grows from 40,960 to 45,056 bytes, changes pages 3, 4
and 7, and adds property LVAL page 10. TCAC0003's complete property/allocation
proof uses checked resizable V2 journal recovery. Frozen TCAC0001/TCAC0002 retain
their original no-default meanings. An acknowledged ColumnAdded receipt alone
does not publish a refreshed document: the complete expected image must reopen.
A separate insert checks omitted zero-default behavior while retaining NULL and
supplied values. SDK comparisons and zero dates do not establish native Access
save timing or timestamp parity.

## Exact Number declaration and persistence

The typed `CreatedDefaultZeroColumnRequired` declaration keeps absent Required
metadata distinct from stored Boolean false. The Number service explicitly
requests `StoredFalse` together with an Int32 zero default. Three independent
[SDK observations](../../../tests/results/schema/add-column-number-sdk-source-2026-11-08.json)
cover an ID-only source, an existing default-only source and a source with
Required false plus default zero. Complete candidate images match all three.
The property writer retains dictionary order: newly introduced Required follows
an existing DefaultValue entry, while an existing Required-first dictionary
remains in that order. Ordinary constructive creation keeps its original order.

`TCAC0003` binds the exact declaration, original image length/digest and bounded
names. Its native plan reconstructs the entire candidate before a checked
resizable V2 journal; cold and settled recovery decode that exact language before
reconstruction. Frozen TCAC0001/0002 remain separate no-default languages.
The Access adapter preserves the retained exclusive owner, original lengths,
explicit cancellation, separate normal native/proof pools and schema-only
ColumnAdded receipts. Recovery spends its original cumulative caller allowance.

Runtime exposes `ReviewAddNumberColumn`, a consuming `ReviewedNumberColumn`,
`SaveWorker::start_number_column` and the existing complete-image guarded
`OpenColumnAdded` read. Review holds no write lease. Actual native review/save
rechecks the source and requested declaration; a terminal receipt alone cannot
publish a refreshed document. The [batch record](../../../tests/results/schema/number-column-runtime-batch-2026-11-08.json)
retains 65 passing checks and strict affected-package lint. The actual GUI draft
producer, not only hand-authored declarations, matches both source classes.

Saved-datasheet naming/save, multi-tab checked publication, close confirmation,
readonly inspection cancellation and explicit failed-refresh Return are connected.
Return preserves the field name, save outcome and cleanup notice without rollback
or replay, and blocks mutation against the stale document until a fresh open.
The actual GUI Number-save/New-row case verifies generated ID and an omitted
Score stored as zero after native reopen; separate runtime cases retain explicit
NULL and supplied-value observations. TCAC0003 retains its original inline/absent
property source; the repeated external-property route below is a separate captured
purpose. Required true, broader properties, populated sources and native Access
timestamps remain outside both profiles. SDK comparisons and macOS checks do not
qualify Microsoft Access or Windows execution.

## Repeated Number addition with existing external properties

The distinct `RepeatedDefaultZeroAddColumnCandidate` and `TCAC0004`/profile 4
original intent admit the same explicit nullable LONG, stored Required(false)
and typed zero declaration on the output of the first Number save. The
[three independent SDK observations](../../../tests/access/schema/number_column_repeat/README.md)
match complete native candidates: the file stays 45,056 bytes, only catalog page 4,
table-definition page 7 and existing property LVAL page 10 change, and allocation
maps and the primary INDEX page remain exact. The new complete MR2 body appends
as row 1; the prior row 0 body remains byte-exact. Dictionary/group order and absent
versus explicit Required metadata remain distinct.

Source admission is positive complete-image reconstruction, not a creator flag,
fixture hash or normalized copy. The exact empty ACE16 generated-key layout
retains owned property pages [10] in an eleven-page file, or [10, 11] in a
twelve-page file after one rollover. Only the last owned page is a candidate.
For n current fields and r occupied property rows summed across that actual
ledger, reconstruct the canonical n-r field prefix, apply the frozen first
Number constructor, then replay each historical append, including its exact
fitted or one-rollover placement. Every historical appended field requires exact
Int32, stored Required(false) and zero policies. The complete resulting source must equal the actual original,
including catalog logical/hidden roots, descriptor selection of the last body,
old property rows, maps, index ownership and untouched bytes. Work and allocations
consume one cumulative native allowance across all stages.

Candidate construction uses the existing declaration/MR2 serializer, catalog
logical-root/record-placement owner and shared `owned_lval` append framing. Exactly
one new body appends to the last owned page when it fits. If page 10 cannot
fit it, exactly one immediate-EOF page 11 may be allocated; subsequent bodies
must fit page 11. The old page 10 stays entirely exact during rollover. A full
page 11 refuses: no second allocation, reuse, reclaim or history retirement is
admitted. The existing sixteen-field bound remains independent: a qualified
source with fifteen fields can yield sixteen; a sixteen-field source cannot
add another field.

The represented EOF policy is explicit. Preview allocation still requires its
original free-cell state. This canonical property route requires the represented
EOF free bit clear, while separately proving physical file length, immediate EOF,
ordinary/LVAL nonownership and the complete constructive source. The bit alone
grants no ownership. Rollover derives exactly two selected owned/candidate map
byte edits, rejects role-byte aliasing and leaves global allocation bytes exact.
Fresh native reopen verifies the exact complete property body and field/index
identity; every earlier row's slot word, span and payload remains unchanged.
Complement proof permits only the selected definition/catalog/property scopes
and those derived map bytes. Additional tables/options insertion, unknown
histories, populated sources, chained/other external storage and growth beyond
page 11 remain unsupported.

TCAC0004 binds the original complete length/digest and exact names; decode returns
request data only. Fresh reconstruction and both-image qualification use separate
normal native/proof allowances and one cumulative recovery allowance. It reuses
the checked resizable V2 journal for both fitted equal-length images and the
one-page rollover; request version and journal version are independent. Extending
source/placement admission changes neither the original declaration nor its
TCAC0004 framing. TCAC0001/0002/0003 retain their exact previous languages and reconstruct only their captured constructors.
Cold/settled recovery dispatches the exact version with no decoder fallback.

The legacy runtime service retains `ReviewAddNumberColumn`,
`ReviewedNumberColumn`, `start_number_column` and `OpenColumnAdded`. The unified
UI uses `ReviewAddField`, `ReviewedFieldAddition` and `start_field_addition` with
the same checked reopen boundary. An engine-owned
metadata observation resolves the actual catalog logical root (including
forwarding), native field framing and LvProp carrier. Review and availability
select one constructor from that fact, then fully qualify it; failed qualification
never retries another profile. Known older-format Source refusal remains
unavailable, while malformed/cancel/resource errors propagate. The worker retains
the actual sealed purpose, same exclusive offline lifecycle, schema-only
ColumnAdded receipt and full-image witness before publishing the fresh document.

The [coordinated repeated-add record](../../../tests/results/rows/repeated-number-workflows-2026-11-08.json)
retains 127 selected passing checks across engine, adapter, runtime and desktop.
It includes three ordinary Click-to-Add/name/Save operations in one session,
followed by actual GUI insertion of a row with generated ID and three omitted
fields stored as zero. A near-capacity history save reaches fourteen readable
fields using the existing runtime native pool; no resource allowance changed.
Strict affected-package lint passes with all features and for normal
creation+journal without preview. This is bounded repeated ADD, not saved-field
rename/drop, general column CRUD or Access 97–2019/native Microsoft Access
qualification. The earlier native and consumer checkpoints, zero-match selection
attempt and corrected test-helper build failure remain separately recorded.

### One-rollover recovery and consumer evidence

The earlier three second-ADD SDK observations above remain same-page evidence.
The later [rollover SDK record](../../../tests/results/schema/number-column-rollover-sdk-2026-11-08.json)
separately pins the complete N13→N14 image, which grows from 45,056 to 49,152
bytes. N15's fitted page-11 construction has independent raw-model/native proof;
it is not an additional SDK execution observation.

The [rollover workflow record](../../../tests/results/schema/number-column-rollover-workflows-2026-11-08.json)
retains 135 selected passing checks: 83 engine, 16 adapter, 18 runtime and 18
desktop checks. Strict normal creation+journal lint without preview passes.
Native checks cover the complete SDK image, positive reconstructed page-11
sources, exact map refusals and full-page refusal.
Checked V2 tests cover committed growth, partial-growth rollback, write-free
settled confirmation and refusal of tampered foreign/global/owned map bytes even
when a generic journal digest is valid. Preview's original RequireFree placement
policy remains separate.

Runtime and ordinary `Desktop::new` headless checks both save N14 using unchanged
normal native/proof/persistence budgets, acknowledge the real actor and pass the
complete-image reopen. The actual GUI then uses New row, omits all fourteen
Number fields and saves generated ID1 plus fourteen non-NULL zeros. No viewport
widening or capability override is used. These checks qualify this bounded
engine/application route; they do not establish native Microsoft Access,
Windows execution, other fields or older-format writes.


## Short Text and mixed-source evidence checkpoint

The [new independent reference](../../../tests/access/rows/short_text_add/README.md) records
eighteen complete SDK/raw AFTER matches for explicit TEXT255 with Required(false),
AllowZeroLength(true), no DefaultValue property and separate compression-off/on
twins; repeated Text and Number-after-Text compositions retain all original
property rows and field facts. A separate third Number SDK image closes the
earlier raw-only history observation. These are new source/target byte oracles,
not native persistence, GUI or native Access admission by themselves. The typed
candidate and version5 request above consume this evidence through separate
source qualification; their workflow checks remain independently accounted.

Actual current TableDraft metadata has the same declared width/policies/default
absence but the existing creator leaves UnicodeCompression off. Microsoft
documents the Access UI compression default as on. The explicit typed native opt-in
now preserves old defaults and images while making that new target explicit.
Source reconstruction preserves each original field's actual compression and
AZL/default presence. The evidence-only checkpoint did not change a wire language;
the separate TCAC0005 operation above binds its own full typed request. No generic
raw flags or silent global default flip is supplied.
