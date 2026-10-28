# Stored attachment FileData queries

Explicit native `query run --collections` exposes the actual attachment
`FileData` member as nullable Binary, alongside the five metadata members:

```sh
tablecraft query run 'SELECT ID, Files.FileName AS Name, Files.FileData AS Data FROM Collections' --database copy.accdb --collections
```

The default result is the exact stored encoded envelope. It may contain a
compressed wrapper and is generally different from the original file. CLI TSV
prints those bytes as `0x` hexadecimal. No decoder, file extraction, URL access,
MIME inference or active content runs. Stored NULL, present empty bytes and an
empty parent's NULL extension remain distinct internally. Empty or unknown
wrappers are valid opaque Binary values; wrapper integrity is only checked by
an explicitly requested decoder.

`--decoded-attachments` continues to select the existing explicit decoded Binary
view of the original files. It never changes the original database. This option
changes FileData's representation, not its nullable Binary declaration, and does
not load payloads when the query only requests metadata. Library hosts use
`NativeSource::with_collections()` for stored bytes and explicitly enable
`with_decoded_attachments()` for decoded bytes.

Binding reads actual hidden-field declarations without scanning records. Native
`inspect_attachment_members` and `inspect_attachment_declarations` expose all six
members; `AttachmentColumn::stored_value` borrows exact stored values. The narrower
`metadata_value` accessor still excludes FileData. FileData has no text comparison
policy. Bare attachment fields and matching wildcards remain refused.

Execution retains the existing complete-snapshot boundary: native storage, parent
keys, child identities, declared carriers and selected payload references are
checked before publication. TOP and filters do not hide a late native storage
failure. All selected members of the same collection use one full snapshot when
FileData is referenced, regardless of projection order. Metadata-only snapshots
remain separate and omit external payload pages; a failed payload read cannot
poison the complete metadata cache. A completed full snapshot can supply later
metadata or raw reads without rereading child records. Decoder activation reuses
that snapshot but validates every requested decoded collection before publication.

Native read limits, cumulative budgets, cancellation, query/output limits,
selected-index refusal and unchanged-source requirements apply to stored bytes.
The decoded-file ceiling does not limit opaque stored values; the native encoded
long-value and complete snapshot limits still do. No new mode, dependency or write
path is introduced.

An explicit parent index can now be combined with stored attachment members.
The [indexed collection contract](../queries/INDEXED_COLLECTION_QUERIES.md)
validates logical root alignment before publication and retains metadata-only
payload deferral, complete collection checks and required-index failures.

## Access-generated attachment child layouts

The contextual reader also supports the observed ACE 16 attachment layout with
variable four-byte Int32 foreign-key and identity fields. Their native flags
remain `0x02`/extended `0x08` and `0x06`/extended `0x04`, respectively; the six
attachment payload fields retain extended flag `0x10`. The earlier fixed-key
layout remains supported.

Admission requires a complete resolved attachment mapping, the empty six-member
conceptual template, exactly two distinct child reference fields, matching native
types and lengths, and the qualified field flags. Ordinary scalar inspection
continues to report these raw flagged fields as unsupported. The attachment
reader supplies separately qualified carriers to the existing cursor; it does
not clear flags or invent a different physical layout. Variable reference bytes,
NULL masks, complete live counts and parent/child membership remain checked.

Both attachment views expose `member_type()` for their certified declarations.
The metadata-only view declares FileData as Binary while omitting its value;
this does not validate or decode unread payload bytes. Adapter cache validation
uses those certified member types and retained text-sort identities, rather than
reinterpreting raw flags as an ordinary scalar declaration. Unrelated six-field
complex templates retain their generic raw-link behavior.

## Evidence and limits

Microsoft's [DAO attachment guide](https://learn.microsoft.com/en-us/office/vba/access/concepts/data-access-objects/work-with-attachments-in-dao)
identifies FileData as internal binary attachment storage. A specific
[Microsoft support query example](https://learn.microsoft.com/en-us/answers/questions/4766187/adding-multiple-attachments-causes-form-duplicates)
describes selecting an attachment's `.FileData` attribute and expanding one record
per attachment. These support query accessibility and the child-row boundary;
they do not establish exact native SQL or DAO Value bytes.

The original [physical oracle](../../testing/AttachmentStoredDataOracle.java) separately
checks the actual hidden FileData declaration is OLE, its flat stored carrier is
`byte[]`, and each raw child value matches Jackcess 5.0.4's public
`getEncodedFileData()` by child identity. The
[query replay](../../testing/compare_attachment_queries.py) compares these exact bytes,
alignment, counts, empty-parent extension and reordered projections. It also
compares explicit decoded bytes with the separate public `getFileData()` getter.
A disposable copy with a changed checksum remains readable as stored Binary while
explicit decoding refuses it. Original fixture and fixed CLI hashes are checked. The
[24 October 2026 report](../../testing/results/stored-attachment-queries-2026-10-24.json)
records 198 passing controls under each of ordinal and stored database comparison
policies (396 total), with unchanged input, oracle, source and immutable CLI hashes.
Fresh original fixtures cover ACE 2007, ACE 2016 and ACE 2019 file families.

This is a documented stored-value query contract, with independent physical
reader evidence. Exact SQL/DAO byte equivalence, negative/empty membership rules
and Windows ACE execution remain unqualified until native comparisons. Existing
query collection expansion and parent-presence semantics continue unchanged.
