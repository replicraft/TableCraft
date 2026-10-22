# Stored List Definition schema agreement

ACCDT List Definition tables now enter the existing table browser and query source
when their stored SOAP declarations agree with the supported inline-XSD profile.
This extends the earlier generic Object-table reader; it does not instantiate a
SharePoint list, contact a service or execute template application content.

The reader validates a bounded declaration sequence and compares its final fields
with inline XSD before returning columns or rows. Basic object/package inventories
remain inert. The usual commands apply without a new flag:

```sh
tablecraft inspect example.accdt --table Records
tablecraft tables example.accdt --count
tablecraft rows example.accdt Records
tablecraft query run 'SELECT ID FROM Records WHERE Flag' --database example.accdt
```

## Declaration profile

The List Definition part must be application/xml, with exactly one related Object
Properties part of the documented content type. That properties body stays opaque;
its application semantics are not inferred. Existing object metadata/name/data-part
requirements still apply. Both schema discovery and complete table reads check
agreement under the caller's shared budgets and cancellation.

The supported definition is Root containing one initial SOAP 1.1 AddList, followed
by UpdateList requests. AddList requires a matching listName and a decimal int32
nonnegative templateID. Other AddList values are inert, as specified by MS-ACCDT.
No default fields or types are synthesized from templateID. Every stored field
needs an explicit complete declaration, including base fields first supplied by
updateFields. Full template/default instantiation remains unqualified.

Each UpdateList validates request child order and requires listProperties,
newFields and updateFields. Optional listName, deleteFields and listVersion remain
within that order. Stored listName does not select an external provider; MS-ACCDT
scopes these requests to the list created in this part. List properties, defaults
and validation expressions remain inert metadata. SOAP headers, other operations,
unsupported structural children and malformed request sequences fail.

Field operations are applied in order. New declarations, partial property updates,
display-name changes and identity-only deletions work. UUID field IDs use the
existing uuid parser, so supported case/braced spellings identify the same field.
Internal names and display names retain the shared ASCII-insensitive identity
policy; Unicode normalization or locale folding is not inferred. Ambiguous matches,
duplicate field identities or method IDs within a Fields container, conflicting UUID replacement, unsupported internal
name changes, unknown deletions and incomplete first declarations fail.

Agreement requires a one-to-one match between final declarations and stored XSD
fields. Display names can match decoded XML names; internal names can match raw
or decoded names. Matching both aliases must still resolve one unique field. An
extra declaration cannot disappear because it is hidden; Hidden is a presentation
flag, not permission to omit a SQL field. Missing/extra/default-dependent fields
remain refused until their schemas can be represented correctly.

| Declared field type | Required inline XSD type |
| --- | --- |
| Text / Note | string |
| Boolean | boolean |
| Counter / Integer | int |
| Number | double |
| Currency | decimal |
| DateTime | dateTime |

An explicit MaxLength must agree with the inline maximum. Required=TRUE cannot
advertise nullable stored cells; FALSE does not require a snapshot to contain NULL.
Counter cells must be nonnullable. Calculated/lookup/collection/unknown field types
remain unsupported rather than being projected as NULL or evaluated. All ordinary
[inline-XSD value checks](ACCESS_TEMPLATE_DATA.md) still apply. These are stored
snapshot types: Currency remains the existing XSD Decimal carrier, not a claim
of complete native Access Currency subtype fidelity.

## Limits and qualification

Definitions reuse bounded UTF-8 XML trees: 8 MiB, 64 levels, 100,000 nodes and
64 attributes per element. There are at most 128 requests and 256 live declarations;
field names are capped at 1,024 UTF-8 bytes and type spellings at 64. Namespace/name
copies and parser work share existing quotas. Field matching, updates and identity
checks meter work and cancellation. Failure discards the local declaration state;
no partial metadata/snapshot is published and no original bytes change.

Binding checks declarations and only the inline schema prefix, without requesting
a record event. An invalid later record can still bind and then fail execution
before that table delivers rows. Operational interruption is not cached as a
permanent schema failure. This introduces no writable capability or permission to
skip data macros, constraints, preservation or native save qualification.

Six feature tests, including generated final-update models, cover successful
renames/base declarations/UUIDs/deletions, mismatched and ambiguous schemas,
unknown calls/default-dependent fields, metadata-only binding, shared limits and
every observed cancellation checkpoint. The original [Python SOAP/XSD runner](testing/compare_template_lists.py)
passes 36 CLI/query controls over stored/deflated original packages and all eight
supported declared field types. Existing typed-template and package runners retain
their 48 and 30 controls. Only TableCraft executes SQL; no SOAP server or application
runtime is involved.

This is partial List Definition support. Broader CAML types and defaults, additional
schema shapes, missing/exported-field representations, Office property semantics
and Access-created save/instantiate comparisons remain required. It does not
complete ACCDT or native Access compatibility.

References: [MS-ACCDT List Schema](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/63e8ce75-d4d4-43f8-91ee-8290ec898a06),
[List Data agreement](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/606146d3-514f-4555-9d7e-039be4b3f40c),
[UpdateList](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-listsws/05a1d418-a679-4751-8a26-6629e8588aaf)
and [UpdateListFieldsRequest](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-listsws/80ce8816-6d1f-4d7e-bab8-632097c14cd2).
Implementation and fixtures are original; no specification schema/example or
third-party binary is copied.
