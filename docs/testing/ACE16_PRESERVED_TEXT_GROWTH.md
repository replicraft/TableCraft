# Growing Short Text copies in mixed schemas

`NativeImage::preview_ace16_preserved_text_growth` edits one ordinary, unindexed
Short Text column outside calculated dependencies in a preservation-qualified
mixed schema. Existing `SlotScalarUpdate` requests carry source-local row ordinals
and exact Text/NULL expectations. The result is a separate non-journalable
engineering candidate; it grants no normal ACE save or macro permission.

The [fitting service](ACE16_PRESERVED_SHORT_TEXT_UPDATES.md) and Default workflow
retain their existing physical-capacity boundary. This growth service is an
explicit choice. The existing [SQL](../access/writes/SQL_ENGINEERING_COPIES.md)
and [typed engineering-copy](../access/writes/ENGINEERING_COPIES.md) workflows
select it with `--variable` for one admitted projected Short Text target. Default
remains fitting; combined INDEX growth, LVAL and larger-image modes retain their
separate restrictions. A failed mode is not retried through another writer.

## Placement and preservation

Changed values use the existing minimum-size text serialization policy. A record
that still fits retains its physical footprint. A larger record becomes a hidden
record on an admitted owned DATA page or a new contiguous EOF DATA page. The
original logical root forwards to that record, so existing index references and
snapshot-local row order remain stable. An already forwarded root is retargeted
directly; its abandoned terminal record remains unchanged.

EOF allocation changes only already represented ordinary-owned, candidate-hint
and GLOBAL bitmap cells. No map storage, map-pointer rewrite, arbitrary in-file
free-page reuse, INDEX or LVAL allocation is admitted. Existing indexes, keys,
histories and declared live-row counts remain exact. Multiple selected rows are
processed in original live-ordinal order, independently of caller request order.

Each final record preserves every unselected fixed field, mask bit and raw
variable payload, including calculated caches and Memo/OLE/Hyperlink descriptors.
Payloads may move inside a rebuilt record without changing their bytes. Raw Text
and NULL no-ops preserve their original representations and inactive bytes.
NULL and present empty Text remain distinct. Required, AllowZeroLength and other
qualified constraints apply to the complete final candidate; a later violation
returns no candidate even after earlier rows required placement.

The complete preserved-source profile still governs schema, properties,
relationships, calculated dependencies, attachment associations, LVAL ownership
and physical body roles. The target metadata predicate alone grants no write
eligibility. Unknown or unsupported content retains its existing refusal. Larger
fixed-image capture policies do not grant this service a larger image ceiling.

## Independent authority

The producer and proof each recapture original roots in source order. The proof
reads only the selected ordinary field, validates the original expectation, and
reconstructs changed records through the separate minimum-size record proof.
It accepts no staged payload, projection or event cache as authority and creates
no placeholder scalar values for unavailable rich fields.

Fresh source-bound metadata protects original roots, terminals and destinations,
including no-ops. Independent represented-map replay checks those proven records
before byte assembly. The actual assembled image then undergoes complete native
capture and preserved-source/constraint admission before a candidate is returned.
Boolean, Int32, fitting Text and ordinary variable writers keep their existing
profiles.

## Observed evidence

Eight independently authored complete-image pairs cover original-page placement,
another owned page, represented EOF allocation, an already forwarded root, mixed
grow/shrink/no-op requests in both caller orders, all no-ops and a separately
framed zero-byte NULL source. Source qualification passes before the first writer:
16 header-6 views are admitted, 16 header-5 rich-source views retain their existing
Extended Date refusal, and three ownership/allocation contradictions are refused.
This is not a general header-5 write restriction.

The first source compilation failed on a test integer type. The next run exposed
a wrong preflight assertion: the unchanged third rich record is 265 bytes, not
365, and original free space is 3381 bytes. Correcting those preflight facts and
adding explicit raw sibling/abandoned-terminal assertions preserved the original
constructor's source-derived byte choices, before any producer call. A later
imports-only cleanup leaves these byte choices unchanged. A private
ordering description was also corrected to source live-ordinal order before
implementation; the reversed mixed expectation contains only one growing row.

The first native writer run passes four functions and all eight full images.
The complete preserved contract passes 41 functions, with two existing helpers
ignored, including cumulative exact-pool/final-cancellation controls and a healthy
AllowZeroLength-false refusal after an earlier row grows. Eleven ordinary variable
regression functions pass. The full preserved frontend targets pass 21 Access
and 19 CLI functions, including 32 new principal complete-image comparisons
through SQL and typed requests. Five causal proof functions accept all eight
literal controls and reject wrong counts, correlated alternative requests, cache
and sentinel changes, root/slot forgeries and unauthorized map edits. Four old
variable-update and seven placement-contract functions also pass. Strict checks
for ACE, Access and CLI across all targets/features, ACE/Access minimal builds,
workspace formatting and the diff check pass. Documentation validation reports
6,760 targets with no missing links. The
[result record](results/ace16-preserved-text-growth-2026-10-30.json) retains the
logs and failure chronology. The genuine fitting corpus does not automatically
qualify these growth paths.
These authored-source checks do not establish native Access save/reopen or
required-macro discovery.

## Independently generated original files

The unchanged [Short Text SDK originals](ACE16_PRESERVED_SHORT_TEXT_UPDATES.md)
also supply a separate growth corpus. Their actual parent DATA pages are 100
(V2016) and 118 (V2019), each with three live roots, 3,816 free bytes, an admitted
candidate hint and allocated GLOBAL membership. The original present Text span
is seven bytes; the NULL span is zero. No source, map hint or old fitting
expectation was changed to make growth fit.

Eight independent complete images were constructed before producer calls:
`alpha` to six snow characters, NULL to six snow characters, and the two-row
batch (`alpha` to six snow characters, NULL to `beta`) in both typed request
orders, on each original. Rebuilt records are 126, 78 and 72 bytes. Appended
hidden slots and forwarding roots retain original logical references; both
batch orders produce the same image in source row order. The proof preserves
actual 27-byte calculated capsules, other rich payloads, every unrelated parent
byte, all other pages, maps, indexes and EOF bytes.

The pure constructor passes all eight cases without child or writer calls.
A separate ignored native SOURCE function directly admits all eight expected
images through the private complete preservation owner before any producer.
A fresh build from committed producer `109670565260214c9b0c5b5a44e702e30ab45639`
then matches all eight complete images on its first attempt: four SQL singles
and four typed batch calls, matching 3,948,544 bytes. Eight fresh unchanged
independent SDK getter calls pass on the first attempt, matching every expected
tagged inventory with only the selected cells changed. Original hashes remain
unchanged. This qualifies same-page placement on these two original profiles;
genuine-file EOF allocation is not established by this corpus.
The [genuine-image result](results/ace16-preserved-text-growth-genuine-images-2026-10-30.json)
keeps source, expected, build, producer and reader phases distinct. These
engineering copies do not qualify native Microsoft Access saves, required-macro
discovery or normal ACE writes.

Saved-only verification passes with 494 artifact pins, two original files, eight
images, four SQL singles, four typed batches and eight SDK reads, launching no
child processes. Its first invocation stopped before data checks on a stale
verifier-code pin; refreshing that metadata retained all model and database
bytes. The expected-source helper passes ACE all-target/all-feature strict
checks and workspace formatting. Final documentation checks are recorded in
the result separately.
