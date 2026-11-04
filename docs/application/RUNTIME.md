# Read sessions and application tasks

`tablecraft-runtime` owns the desktop's working native read session, command
execution and background worker. It has no egui/eframe dependency. The desktop
owns native windows and dialogs, forwards commands and moves owned results into
the shared UI's presentation types. Engine interpretation stays in ACE, Access
adapts it, and query evaluation stays in the query libraries.

## Session and task ownership

Open captures an immutable image of a stable/offline file up to 64 MiB, closing
the original handle after the bounded copy. It checks length before and after;
this does not establish a transactional snapshot against same-size concurrent
writes. Close other writers first. Package selection and native password/page
decoding use the existing Access/ACE services. Passwords are consumed into
zeroizing storage, including on failures. Opening a source never executes its
active content or follows stored external paths.

The worker owns the session and one replaceable pending request and response.
New commands supersede older work through generation cancellation; publication
and host acceptance both reject stale generations. Pending replacement and old
result clearing share the mailbox lock; backend work runs outside it. Dropping a
worker requests cancellation without joining on the GUI thread. A blocking OS
file-open/read call must return before a cooperative cancellation check can run.

| Command | Owned response and scope |
| --- | --- |
| Open | Inert catalog and format; a failed replacement clears the old session |
| Page | One buffered table page with typed scalar/stored-field cells |
| Describe | Field declarations and reader facts, without user-record scans |
| Query | Explicit read-query result over the selected source; no implicit provider opens |

Describe's reader NULL policy is separate from Required/default/validation
properties, which it does not inspect. Unknown field declarations remain visible.
Metadata does not establish whole-table readability or permission to write.

## Table cells and stored collections

Page uses `NativeBrowser::browse_page` once. Metadata selects ordinary scalar
paging or the existing qualified stored-field profile. Failed reads are not
retried as another profile. Unknown, corrupt or unqualified columns refuse the
operation rather than disappearing from the page or becoming empty collections.
Checked page EOF controls the Next action; page size and declared row count do
not substitute for it.

Each cell owns its display text and explicit reader facts:

- Scalar cells carry independent NULL and stored-calculation flags. Literal text
  `NULL` does not acquire NULL semantics from its spelling. Cached NULL results
  retain both flags; no formula evaluates and freshness remains unknown.
- Multivalue, attachment and stored-history cells carry assigned-key presence and
  a complete member count. An absent key differs from an assigned empty list.
  Duplicate and NULL elements count. An attachment whose FileData is NULL remains
  a member. All typed items must validate before the count is published.
- Current Memo text remains a separate scalar from stored version membership.
  Display does not select a latest version or infer timestamps from ordering.

The current DTO contains collection summaries, not child values or attachment
payloads. Explicit child inspection is still work. The underlying stored-field
reader is eager: it validates complete parent/child snapshots and retains encoded
attachments before returning a visible page. This is bounded complete-read
behavior, not lazy expansion or constant memory. No attachment decoder, URL
resolver, file export or external application is invoked by browsing.

The shared UI renders values before compact annotations, retains provenance in
tooltips/accessibility context and copies the complete display text without its
badge. Display strings are not lossless exports, edit carriers or native field
formatting rules. Query Empty/Missing/error states remain explicit display
states; they are not silently converted to stored NULL.

## Limits and verification

Each native operation currently receives 512 MiB of charged bytes, 100,000 rows
and 5,000,000 work steps. Visible pages request 100 records. Stored-field pages
also retain the existing default complete parent/child snapshot ceiling of
16,384 rows; a small visible page does not bypass that ceiling. Ordinary scalar
pages keep their paged profile instead of taking that complete snapshot.

Query execution independently uses 16 MiB, 10,000 rows and 1,000,000 steps, with a
64 KiB SQL input ceiling. Display publication has an 8 MiB budget and 100,000
steps, including typed row/cell envelopes and collection iteration. These are
consumer controls, not format limits or an exact process-heap cap. Partial
pages/counts are not published after failure. Generation checks remain necessary
even when cancellation is requested.

Feature-local runtime tests use the public Worker and existing testkit inputs.
They cover immutable reads, failed opens/password retry, real queries, metadata,
stored caches, collection presence/counts, corrupt child refusal and mailbox
ordering/cancellation. Shared UI tests exercise copy, focus, provenance and
virtualization; these do not qualify native screen readers or Access behavior.
Native observations and current platform limits are recorded in the
[desktop guide](../ui/DESKTOP.md). Payload-free diagnostics remain governed by
[the tracing contract](../development/TRACING.md).
