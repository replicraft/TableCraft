# Testing and evidence

- [Cross-platform audit and native CI scope](CROSS_PLATFORM_AUDIT.md)
- [Read/query acceptance](READ_QUERY_ACCEPTANCE.md)
- [Fixture provenance and notices](FIXTURES.md)
- [Shared test fixtures and support](TESTKIT.md)
- [Append-table independent-reader observations](results/ace16-add-table-independent-reader-2026-11-07.json)
- [Repeated append-table independent-reader observations](results/ace16-repeated-table-independent-reader-2026-11-07.json)
- [Manual/keyless Design independent-reader observations](results/ace16-manual-design-independent-reader-2026-11-07.json)
- [Description creation independent-reader observations](results/ace16-description-independent-reader-2026-11-07.json)
- [Shared indexed GUID fixture extraction](results/shared-indexed-guid-fixtures-2026-11-03.json)
- [Normal indexed GUID write acceptance](results/ace16-normal-indexed-guid-update-2026-11-03.json)
- [Normal unindexed GUID write acceptance](results/ace16-normal-guid-fixed-update-2026-11-03.json)
- [GUID UPDATE sources and admission](ACE16_GUID_UPDATE_ADMISSION.md)
- [Retained GUID/Int32 tuple Source qualification](results/ace16-retained-guid-tuple-source-2026-11-03.json)
- [GUID/Int32 tuple UPDATE baseline](results/ace16-guid-tuple-update-baseline-2026-11-03.json)
- [Indexed GUID UPDATE expected-source qualification](results/ace16-indexed-guid-update-sources-2026-11-03.json)
- [Retained GUID Source admission](results/ace16-retained-guid-source-2026-11-03.json)
- [Byte/Int16 literal UPDATE source qualification](ACE16_BYTE_INT16_UPDATE_SOURCES.md)
- [Normal Byte/Int16 write acceptance](results/ace16-normal-byte-int16-update-2026-11-03.json)
- [Existing Single/Double UPDATE source qualification](ACE16_IEEE_UPDATE_SOURCES.md)
- [Normal exact Single/Double write acceptance](results/ace16-normal-ieee-update-2026-11-03.json)
- [Generated-field UPDATE source qualification](ACE16_GENERATED_COMPANION_UPDATE_SOURCES.md)
- [Retained Binary-index field UPDATE sources](ACE16_RETAINED_BINARY_MARKER_SOURCES.md)
- [Test duplication audit](TEST_DUPLICATION_AUDIT.md)
- [Wider numeric root-promotion sources](ACE16_WIDE_ROOT_PROMOTION_SOURCES.md)
- [Wider numeric keys with ordinary siblings: originals](ACE16_WIDE_SIBLING_ORIGINALS.md)
- [Existing ignored-NULL source admission audit](ace16_fixed_ignore_null_updates/NORMAL_ADMISSION.md)
- [Normal single-Int32 ignored-NULL update acceptance](results/ace16-normal-ignore-null-update-2026-11-03.json)
- [Wider numeric keys with ordinary siblings: expected updates](ACE16_WIDE_SIBLING_AFTER_SOURCES.md)
- [Int32 key updates beside retained Binary indexes: sources](ACE16_RETAINED_BINARY_ID_UPDATE_SOURCES.md)
- [Access application corpus policy](ACCESS_APPLICATIONS.md)
- [Multiple-index INSERT engineering profile](ACE16_MULTIPLE_INDEX_INSERTS.md)
- [Physical secondary-index creation SOURCE qualification](ACE16_SECONDARY_INDEX_CREATION.md)
- [Explicit larger fixed-update image capture](ACE16_LARGER_FIXED_IMAGES.md)
- [Explicit BeforeChange field-update candidates](ACE16_BEFORE_CHANGE_FIELD_COPIES.md)
- [Complete selected before-event collections](ACE16_BEFORE_EVENT_COLLECTIONS.md)
- [Preserved mixed-schema Int32 updates](ACE16_PRESERVED_INT32_UPDATES.md)
- [Fitting Short Text updates in mixed schemas](ACE16_PRESERVED_SHORT_TEXT_UPDATES.md)
- [Growing Short Text updates in mixed schemas](ACE16_PRESERVED_TEXT_GROWTH.md)
- [Verification records](results/)

This directory also owns focused test contracts, reference runners and inert
fixture generators. Each document states its evidence and qualification limits.
Native Microsoft Access comparisons remain distinct from independent-reader and
headless checks.

Historical records in `results/` retain their captured paths, hashes and revision
identifiers. Those paths describe the recorded revision; they are not rewritten
when current documentation moves. Fixture, runner and release-note paths remain
stable during documentation-only reorganizations.

[Documentation index](../README.md)
