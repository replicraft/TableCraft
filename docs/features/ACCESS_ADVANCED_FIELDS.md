# Advanced Access field behavior

Status: inert field-feature metadata is implemented; structured child-value
codecs, calculated evaluation and editing remain future requirements. Current core
values represent scalars. No native complex-field codec, field binding or field
editor is implemented. SQL member syntax does not establish `.Value` semantics.

## Contracts beyond scalar storage

| Feature | Behavior that must survive |
| --- | --- |
| Lookup metadata | Stored bound value versus displayed value, source definitions, column bindings and single/multiple selection |
| Multivalued fields | Child identities/values, null/empty distinctions, `.Value` flattening, criteria, grouping and supported mutation rules |
| Attachments | Parent/child records, names/types, original payload bytes, metadata and bounded streaming |
| Rich text | Original markup and formatting metadata, plain-text projection as an explicit conversion |
| Append-only Long Text | Current content and historical entries with qualified update/history access |
| Hyperlinks | Display/address/subaddress/screen-tip components and round-trip encoding |
| Input masks | Display/input rules, literal-storage options and interaction with underlying stored type |
| Calculated fields | Expression/type metadata, allowed functions, dependencies, errors and native recalculation rules |

Keep structured fields out of ordinary scalar coercion. An attachment collection
is not a Binary value and a multivalue is not delimited Text. Native child records
and metadata belong to the engine; schema describes field capabilities; query
binding/evaluation implements access and row expansion; DAO exposes child cursors;
forms/designers own presentation. Avoid a single universal Value variant that
imports these whole subsystems into core.

Input masks are not a substitute for engine validation. Qualify which host applies
which rule rather than assuming every DAO/SQL write follows a UI mask. Calculated
field expressions need their own restricted context, not the full set of future
VBA/UDF capabilities. Required validation and data macros must run or writes fail.

Unsupported conversion must identify loss before modifying data: flattening
multivalues, dropping history, stripping markup or extracting only attachment
names cannot be an implicit native save. SQLite conversion needs explicit child
schema and metadata policies; SQLite alone supplies no Access application runtime.

## Acceptance evidence

Use recipe-generated native fixtures for each feature and combinations, then
application copies. Compare Access query row expansion/criteria, DAO children,
attachment hashes, history and metadata before/after supported edits and reopen.
Test malformed children, size limits, failed updates and preservation during
unrelated edits. No advanced-field compatibility is qualified today; record
results in the [application corpus](../testing/ACCESS_APPLICATIONS.md).

## References

Microsoft documents [lookup/multivalue query behavior](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries),
[Recordset2 complex children](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset2-object-dao)
and [Field2.AppendOnly](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-appendonly-property-dao).
See also [attachment behavior](https://support.microsoft.com/en-us/access/attach-files-and-graphics-to-the-records-in-your-database),
[field types/properties](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties)
and [input masks](https://support.microsoft.com/en-us/access/control-data-entry-formats-with-input-masks).
Each field profile needs property-level references and native evidence when
implemented; this inventory does not assert a complete native specification.

## Working inert metadata slice

`inspect_table_features` joins bounded native schema and original property maps on
stable/offline input. `ColumnFeatures` exposes calculation flags/expression/result
native tag, complex-child FK marker, AutoNumber/Hyperlink flags and explicit
AppendOnly/TextFormat declarations. `NativeColumn::complex_type_id` preserves the
raw ACE type identifier without pretending it resolves children; Jet 4 does not
interpret that overlapping definition slot. Missing properties remain unknown,
wrong typed/width declarations fail on interpretation, and unknown bytes/properties
remain retained. No expression, handler, import mask or active content executes.

CLI: `tablecraft inspect COPY.accdb --table NAME --features`. Results are buffered;
malformed typed declarations return no partial stdout. Ordinary scalar reads
continue rejecting calculated/complex layouts, including empty tables.

The original [advanced fixture](../testing/AdvancedFieldFixture.java) generates
V2016/V2019 calculated, multivalue, attachment, append-only/history, rich-text,
hyperlink and input-mask declarations. Each has ten columns including the hidden
history field. [Independent comparison](../testing/compare_advanced_fields.py)
matches all exposed semantic declarations, compares complex presence and verifies
three distinct raw type IDs. Exact numeric IDs have separate synthetic golden
coverage; the independent public API does not expose their raw numbers. The
property stream retains input-mask/lookup and other declarations without claiming
UI or write semantics. Input hashes remain unchanged; Microsoft Access remains
unrun. Structured child references/values and native recalculation remain next.

Encoding references: [ColumnImpl flags](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java),
[JetFormat complex-ID slot](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java),
and [DAO Field2.Expression](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-expression-property-dao).
No implementation was copied; no third-party fixture was added.
