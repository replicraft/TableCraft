# Testing and evidence

- [Read/query acceptance](READ_QUERY_ACCEPTANCE.md)
- [Fixture provenance and notices](FIXTURES.md)
- [Shared test fixtures and support](TESTKIT.md)
- [Byte/Int16 literal UPDATE source qualification](ACE16_BYTE_INT16_UPDATE_SOURCES.md)
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
