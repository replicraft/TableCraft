# ACE 16 data-macro qualification

Status: evidence requirements and current observations, 29 October 2026.
[Inert table Extra framing](../access/schema/NATIVE_TABLE_EXTRA.md) now has a
bounded reader. Semantic data-macro detection, qualified absence and execution
remain unfinished. All production ACE writes remain blocked. This document does
not establish native Access qualification.

## Current evidence and the write gate

The unchanged registered Access example has three ordinary catalog Macro objects:
AutoKeys, Ribbon and zzzAutoExec. Its MSysAccessXML schema has six scalar fields
and zero declared rows; MSysAccessStorage has seven scalar fields and 328 declared
rows. These are inert metadata observations, not proof that any table lacks event
data macros. No application content or macro was executed. Original provenance,
license and SHA-256 remain in [FIXTURES.md](FIXTURES.md).
Its creating Access build is unassessed; these observations do not turn it into
an ACE 16 data-macro fixture.

```sh
tablecraft inspect docs/examples/Access_Example_VBA.accdb --catalog
tablecraft inspect docs/examples/Access_Example_VBA.accdb --table MSysAccessXML
tablecraft inspect docs/examples/Access_Example_VBA.accdb --table MSysAccessStorage
```

The current reviewed Jackcess artifact/fixture workflow supplies no qualified
positive data-macro creation/detection path. Its generated V2016/V2019 headers do
not supply native Access build provenance or prove macro absence. Catalog kinds,
empty XML tables, NULL local streams, missing children and successful scalar/index
reads must not become an ACE write permit.

[Microsoft's desktop documentation](https://support.microsoft.com/en-us/access/add-data-macros-in-a-desktop-database)
distinguishes table-triggered macros from explicitly invoked named macros.
Table event behavior is not established by enumerating ordinary catalog macros.
The existing [write gate](../access/writes/NATIVE_WRITES.md) and [preservation limits](../access/writes/NATIVE_PRESERVATION.md)
therefore remain unchanged while positive native evidence is missing.

## Reference boundaries

### Observed table Extra storage

Desktop-origin examples acquired for inert investigation contain table-owned
`MSysObjects.LvExtra` values with wide-MR2 framing. Their actual stored selector
is ACE 2010, despite newer product/course labels. They are secondary evidence for
discovering the carrier, not native ACE 16 qualification. Original files and
extracted payloads remain private and unchanged.

The bounded reader retains exact table identity, native parent and catalog row
location, raw bytes, numeric groups and property names. It can expose validated
UTF-16 text without parsing or executing XML. Observed property names and event
names differ in some records; neither numeric group kind nor property name alone
identifies an event. Desktop-origin text also uses the Access Services
`2009/11/application` namespace. Namespace alone cannot classify a payload as a
web-only feature.

This advances carrier inspection. It does not establish absence, all supported
namespaces/actions, event order, rollback, or safe ordinary saves. Names-only
Extra values remain names-only observations. Missing or NULL Extra remains a
physical presence fact, not an absence certificate. The existing narrow `LvProp`
reader and normal write gates remain unchanged.

Desktop references describe [Before Change](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-change-macro-event),
[Before Delete](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/before-delete-macro-event),
[After Insert](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/after-insert-macro-event),
[After Update](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/after-update-macro-event)
and [After Delete](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/after-delete-macro-event).
Before Change covers inserts and updates; its old/new values and IsInsert state
need separate cases. RaiseError can cancel Before Change/Before Delete operations.
These documented behaviors require native comparison before TableCraft claims them.

Desktop example XML uses an Access Services `2009/04/application` namespace.
[MS-AXL's published schema](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-axl/9be0c86e-693e-452c-a2cd-cf6aebf83e71)
uses `2009/11/application`; [MS-AXL2](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-axl2/bfdd300c-7b9c-4a9a-8669-c3cad1f3a8ce)
describes Access Services data macros, including event embedding and error rollback.
Those XML/service contracts are useful references, not native ACCDB storage maps
or evidence that desktop exports share every namespace, action or transaction rule.
Record actual ACE 16 exports and namespace/version before implementing their decoder.

## Required original fixture set

Create fresh databases in a provisioned Windows environment with Microsoft
Access/ACE 16. Record exact product/engine builds, bitness, locale/collation and
the observed file selector independently; engine version is not a header family.
Use original simple tables and original macro definitions, without VBA, linked
sources, AutoExec, startup forms, credentials or external effects. Preserve each
original and run scenarios on disposable copies under the [corpus policy](ACCESS_APPLICATIONS.md).

| Case | Evidence to capture |
| --- | --- |
| No macros | Native-created controls, including a compact/reopen control, not merely a Jackcess-created file |
| Before Change | Insert and update separately; old/new values, IsInsert, a deterministic field change and a rejected change |
| Before Delete | Allowed and rejected deletes; retained original row on rejection |
| After Insert / Update / Delete | One event at a time, then combinations; a deterministic audit-table effect |
| Named macro | Stored definition and explicit invocation separately; no automatic event inferred from its presence |
| Failure | Required macro error, nested call failure, cancellation and native rollback outcomes |
| Mutation of definitions | Add, replace, remove, reopen and compact; retain before/after file and export hashes |
| Unqualified content | Unknown action/namespace, malformed or missing body and unsupported dependencies; writes must remain blocked |

Capture full native images and inert catalog/table/property/child-stream inventories
before and after each definition change, plus native macro exports and expected
event/error traces. Correlate changed native carriers with the controlling table
identity across rename/compact/reopen. Do not classify a carrier from one string,
object type, guessed flag or XML table name. No native storage locator is qualified
by the current observations.

## Acceptance before enabling writes

A detector must distinguish positive presence, qualified absence and incomplete/
unknown coverage. Only evidence-backed absence can permit the corresponding
macro-free profile. Present macros need their required semantics and preservation;
unknown or unsupported cases continue to block writes. Absence qualification must
survive definition removal, stale/orphan records, malformed references and all
qualified file layouts, with bounded allocation, I/O, work and cancellation.

Compare explicitly selected native DAO/SQL write paths and save/reopen outcomes;
do not assume form-event behavior equals table-event behavior. Establish native
error/rollback behavior, cross-table effects, constraints, indexes and generated
identities before extending transactions or replay. Unavailable macro behavior
cannot be bypassed by direct row patches. Keep native engineering comparisons
separate from the deferred VBA/COM/Wine application compatibility implementation.

Positive ACE 16 fixtures with recorded native build provenance remain a dependency
for absence and write qualification. The secondary positive storage observations
above support continued bounded decoder work; they do not complete this gate or
roadmap steps 1–9.

## Engineering progress before Windows qualification

The user will test on Windows later and authorized continued independent write
work. The [ACE 16 preview](ACE16_WRITE_PREVIEWS.md) now produces explicit disposable
physical candidates with structural/constraint/byte-preservation checks. This is
not positive macro evidence or an absence detector. Its separate return type and
opt-in feature retain the ordinary ACE write blocker; required macro semantics
and the native evidence cases above remain acceptance work.
