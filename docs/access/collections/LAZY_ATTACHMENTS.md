# Source-bound per-parent encoded attachments

`AttachmentReader::open` accepts a stable/offline native source, catalog scope,
table and attachment column, checked complete-directory limits and the original
cumulative native budget/cancellation callback. It qualifies actual owner,
conceptual template and child metadata, including the existing attachment-specific
modern variable reference fields. Generic scalar table eligibility is unchanged.
All parent keys and child foreign keys, signed identities and logical ROWs are
checked through EOF before return. There is no attachment-specific 100-member
limit borrowed from multivalues.

`parents()` is complete unfiltered membership. `read_parent(ordinal)` returns a
`ParentAttachments` with owned stored metadata and encoded FileData rows in original
child order. Its `attachments()` iterator uses the same `StoredAttachment` decoder
as the full reader. NULL FileData counts as a member; absent parent keys and
assigned empty collections remain distinct. Metadata strings and encoded payloads
remain owned after the reader drops. Logical parent ROW/key identity remains local
to the issuing source and schema, rather than a portable mutation witness.

Opening reads key-containing DATA pages without decoding child metadata/payload
fields or fetching external FileData. Each parent read resolves fresh complete
metadata, rechecks all parent keys, and replays the complete child ROW/FK/identity
census. It decodes payload fields only for the selected parent and publishes no
result until checked EOF. Invalid native LVAL storage on an unselected child is
not loaded; requesting that child exposes the actual storage error. Structural,
metadata and key errors in the complete directory remain errors even for an empty
or unrelated selected parent.

FileData wrapper interpretation is separate. Unknown encoded wrapper bytes remain
stored Binary values. `StoredAttachment::decode` or `decode_with` explicitly uses
its supplied budget/decoder and may refuse unknown wrappers or resource limits.
No automatic decode, file extraction, URL resolution, execution or query evaluation
occurs. The reader's own native pools stay cumulative; explicit decoding does not
silently refill or reuse them.

The browser exclusively lends its actual source and original controls until reader
drop. Each request can rescan all key pages; this is not O(1) lookup, constant-memory
browsing, a native lock/snapshot or coherent concurrent-file support. Invalid parent
ordinals leave a ready reader unchanged. Operational failures return no partial
parent and make later reads return `CollectionReaderFailed`; reopen explicitly to
capture a new directory. Previously returned owned results remain usable.

Existing `AttachmentColumn`, metadata-only attachment readers, default collection
browsing and complete query/presence caches are unchanged. A selected parent result
has no route into those complete caches. This API adds no CLI syntax or native
Access compatibility claim.

Eight new engine and three browser functions pass within 86 engine, 68 adapter
and 35 CLI browsing functions. Strict all-target checks, minimal builds,
warning-free Rust documentation and dependency guards pass. The
[acceptance record](../../../tests/results/reading/lazy-attachment-reader-2026-11-02.json)
retains the first test-only lint failure and lexical-scope correction separately
from native Access qualification.
