# Native Access

Jet/ACE reading, native queries, storage, indexes, preservation and bounded writes.

[Documentation index](../README.md)

- [Read-only native table consistency checks](NATIVE_CHECKS.md)
- [Read-only native format inspection](NATIVE_READING.md)

## Calculations

- [Explicit cached calculation queries](calculations/CACHED_CALCULATION_QUERIES.md)
- [Cached Decimal calculations](calculations/CACHED_DECIMAL_CALCULATIONS.md)
- [Cached Long Text calculations](calculations/CACHED_MEMO_CALCULATIONS.md)
- [Explicit row-local calculation previews](calculations/CALCULATION_PREVIEWS.md)
- [Explicit recomputed calculation views](calculations/RECOMPUTED_CALCULATION_VIEWS.md)

## Collections

- [Explicit larger decoded attachment files](collections/LARGER_DECODED_ATTACHMENTS.md)
- [Explicit larger native collections](collections/LARGER_NATIVE_COLLECTIONS.md)
- [Stored attachment FileData queries](collections/STORED_ATTACHMENT_DATA.md)
- [Explicit stored Memo history queries](collections/STORED_HISTORY_QUERIES.md)

## Containers

- [Bounded ACCDC read-only spooling](containers/ACCDC_SPOOLED_READS.md)
- [Inert ACCDT package inspection](containers/ACCESS_TEMPLATES.md)
- [Stored ACCDT table data](containers/ACCESS_TEMPLATE_DATA.md)
- [Stored List Definition schema agreement](containers/ACCESS_TEMPLATE_LISTS.md)

## Indexes

- [Automatic native index selection](indexes/AUTOMATIC_INDEXES.md)
- [Exact Currency index selections](indexes/CURRENCY_RANGES.md)
- [Calendar-safe native DATE candidates](indexes/DATE_INDEX_CANDIDATES.md)
- [Exact Byte and Int16 index ranges](indexes/NARROW_RANGES.md)
- [Native index inspection](indexes/NATIVE_INDEXES.md)
- [Native text ordering](indexes/NATIVE_TEXT.md)
- [Comparer-qualified Short Text equality seeks](indexes/TEXT_EQUALITY_SEEKS.md)

## Profiles

- [ACE 16 native compatibility work](profiles/NATIVE_ACE16.md)
- [Jet 4 current-profile qualification](profiles/NATIVE_JET4.md)

## Queries

- [Explicit larger native scalar query snapshots](queries/LARGER_NATIVE_QUERIES.md)
- [Explicit native external database sources](queries/NATIVE_EXTERNAL_QUERIES.md)
- [Explicit one-hop native linked reads](queries/NATIVE_LINKED_QUERIES.md)
- [Native progressive scalar query sources](queries/NATIVE_PROGRESSIVE_QUERIES.md)
- [Read-only queries over native scalar tables](queries/NATIVE_QUERIES.md)
- [Native saved crosstab reads](queries/NATIVE_SAVED_CROSSTABS.md)
- [Stored derived-query sources](queries/NATIVE_SAVED_DERIVED.md)
- [Saved query external database sources](queries/NATIVE_SAVED_EXTERNAL_QUERIES.md)
- [Native saved-query join forests](queries/NATIVE_SAVED_JOINS.md)
- [Stored saved-query identifier spellings](queries/NATIVE_SAVED_NAMES.md)
- [Native saved-query fragment ordering](queries/NATIVE_SAVED_ORDERING.md)
- [Inert saved-query fragments](queries/NATIVE_SAVED_QUERIES.md)
- [Native saved UNION reads](queries/NATIVE_SAVED_UNIONS.md)
- [Cumulative native allowances across explicit sources](queries/NATIVE_SOURCE_BUDGETS.md)

## Schema

- [Explicit native record constraints](schema/NATIVE_CONSTRAINTS.md)
- [Inert native linked-table definitions](schema/NATIVE_LINKED_TABLES.md)
- [Native property inspection](schema/NATIVE_PROPERTIES.md)
- [Native relationship declarations](schema/NATIVE_RELATIONSHIPS.md)

## Storage

- [Explicit larger encoded native values](storage/LARGER_ENCODED_VALUES.md)
- [Explicit larger decoded Memo reads](storage/LARGER_MEMOS.md)
- [Larger native stored payloads](storage/LARGE_PAYLOADS.md)
- [Native encryption](storage/NATIVE_ENCRYPTION.md)
- [High-offset native Memo and OLE read reliability](storage/NATIVE_HIGH_OFFSET_PAYLOADS.md)
- [Native read failure and continuation reliability](storage/NATIVE_READ_FAILURES.md)
- [Native scalar tables](storage/NATIVE_TABLES.md)

## Writes

- [Complete native image retention](writes/NATIVE_PRESERVATION.md)
- [Offline transaction and coordination foundations](writes/NATIVE_TRANSACTIONS.md)
- [Bounded offline native edits](writes/NATIVE_WRITES.md)
- [Bounded query editing](writes/QUERY_EDITING.md)
