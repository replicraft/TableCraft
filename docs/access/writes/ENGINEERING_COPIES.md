# Experimental ACE write copies

For SQL predicates over original rows, use the separate
[SQL UPDATE](SQL_ENGINEERING_COPIES.md) and [SQL DELETE](SQL_DELETE_COPIES.md)
commands. They share the same fresh-copy publication boundary.

The explicit `tablecraft write preview-update`, `preview-insert` and
`preview-delete` commands prepare disposable engineering copies through the
existing typed ACE engine services. They are separate from normal `query update`
and `NativeEditor` admission. Required data macros are not qualified or executed;
these commands do not establish macro absence, Microsoft Access compatibility,
a normal ACE save, or journal permission.

Use a stable, offline source and a fresh output path. The source opens read-only
under the CLI's existing shared OS lock. This does not coordinate Access's live
shared-file protocol. Encrypted databases and deployment packages are refused;
supplying `--password` does not enable an encrypted rewrite or publish a decrypted
plaintext copy. No database content, stored code or providers execute.

## Expected-value UPDATE

```sh
tablecraft write preview-update offline.accdb edited.accdb \
  --set Items Score 0 int32 10 11

tablecraft write preview-update contacts.accdb edited.accdb --variable --index-growth \
  --set Contacts Name 0 text Alice Alexandra

tablecraft write preview-update offline.accdb edited.accdb --variable \
  --set Items Age 3 int32 null 40 \
  --set Items Name 3 text null aa

tablecraft write preview-update offline.accdb binary-edited.accdb --variable \
  --set Items Age 3 int32 null 40 \
  --set Items Name 3 binary null 00ff
```

Each `--set` contains `TABLE COLUMN ROW KIND EXPECTED VALUE`. ROW is the zero-based
original live-row ordinal, not a key value. All assignments refer to one immutable
captured source. Groups are combined before one engine call; a stale expected
value or any later failure returns no copy. Multiple fixed-field assignments can
span qualified tables. `--variable` can combine fixed siblings with exactly one
Short Text/Binary column in one table. Expected carriers and ordinals come
unchanged from the caller, not from a replacement snapshot expectation.
`--long-value` remains single-table/single-column; mixed LVAL and multiple-variable
requests refuse rather than being applied sequentially.

The default uses fitting fixed/batch maintenance. Explicit `--index-growth`
permits the engine's qualified Int32 single/tuple growth profile. `--variable`
selects Short Text/Binary replacement, including qualified DATA forwarding.
With fixed siblings it uses one joint DATA-placement preview: the original page,
another owned DATA page, or qualified represented EOF allocation. INDEX owners
must fit. Mixed `--variable --index-growth` refuses before typed group
serialization; that combination retains the existing qualified single-key
General/Binary index-growth service only for one variable column. `--long-value` selects ordinary Memo/OLE replacement through
existing LVAL ownership checks. Those carriers use `text` and `binary` respectively.
Unknown indexes, collations, maps or record profiles remain typed refusals.

`--cascade-updates` explicitly expands the engine's qualified declared update
actions and conflicts with growth, variable and long-value modes. No failed mode
falls back to a different engine service. The source's actual constraints,
relationships, Required/unique/NULL rules and preservation checks remain active.

For UPDATE, the exact word `null` means NULL. Prefix a literal with `value:` to
force its value: `value:null` is the text "null", `value:` is empty text, and
`value:value:x` is the text "value:x". KIND still names the actual carrier when
NULL is requested. Text arguments use shell quoting; Binary is hexadecimal.

Fixed kinds are `boolean`, `byte`, `int16`, `int32`, `int64`, `currency-scaled`,
`decimal`, `single-bits`, `double-bits`, `date-bits`, `guid` and `extended-date`.
Currency supplies its exact signed scaled integer; Decimal uses `COEFFICIENT:SCALE`.
The bits kinds supply hexadecimal native value bits rather than rounded display
numbers. Extended Date/Time uses exactly `YYYY-MM-DDTHH:MM:SS.fffffff` without a
timezone and remains subject to the ACE2019-only field profile.

## Complete-row INSERT

```sh
tablecraft write preview-insert offline.accdb inserted.accdb Items rows.tsv
```

Input is headerless quoted TSV, with one KIND/DATA pair per column in native
column order. Fields containing tabs, newlines or quotes use CSV quoting with a
tab delimiter and doubled quotes. Every row has the same arity. NULL is a `null`
KIND with empty DATA; `text` DATA containing "null" is literal text. Empty Text and
Binary are distinct from NULL. All values are parsed before opening the database.

For example, a table with Int32 ID, Short Text Name and nullable Int32 Score could
use these six fields per row:

```text
int32	4	text	Alice	int32	25
int32	5	text	Bob	null	""
```

The engine validates the actual schema and requires its qualified primary/index
profile. Values are never defaulted or coerced. Existing generated columns remain
refused by the manual path. Explicit `--incrementing` selects increment-by-one
engineering behavior and obtains the original LAST-issued counter from the same
captured image. Use `--incrementing-last LAST` instead to supply an exact counter
precondition yourself. These flags are mutually exclusive; neither detects stored
Increment/Random mode or derives a seed from live MAX. The existing engine preview
independently revalidates the counter before returning its complete candidate.

Both incrementing modes take companions only, excluding the generated primary
column. Every companion must be supplied, including explicit NULLs. The current
TSV protocol cannot represent a zero-companion row; generated-only tables require
the engine API. Qualified field-level text defaults remain inert when all
ordinary values are supplied; they are never evaluated or substituted for NULL.
Unqualified default declarations and NewValues properties remain refused.
Audited generated column/value facts print only after publication. The control
report is TSV; the global row/table presentation option does not change it.
The counter helper's two source gates and four API tests pass. Its CLI argument
control and actual two-row command workflow also pass, including complete-byte
parity with the existing explicit-LAST path, generated IDs 31 and 32, and stale
explicit-counter refusal without output. This does not establish new native
Access qualification.

## Source-bound DELETE

```sh
tablecraft write preview-delete offline.accdb deleted.accdb Items --row 1 --row 3
```

The engine captures complete original expected record bytes for the requested
live-row ordinals from the same immutable image, then runs the existing DELETE
preview. Duplicate, missing and unsupported roots fail; no second source capture
or candidate chaining occurs. For stronger externally supplied raw preconditions,
use repeated `--delete ROW:HEX` or `--delete-file requests.tsv` containing two
quoted TSV fields, ROW and HEX. These three input modes are mutually exclusive.
HEX includes the complete original record header and null mask, excluding page
and slot directory words. The CLI does not interpret native record layout.

Explicit `--cascade-deletes` selects qualified CASCADE DELETE and SET NULL actions.
Without it, the existing explicit-delete final membership policy applies. No
VBA, data macro or application callback executes.

## Bounds and publication

All request parsing, source capture, engine preparation, result formatting and
output I/O share one operation allowance. Defaults are 256 MiB cumulative bytes,
100,000 rows and 100,000,000 steps, adjustable through `--work-bytes`, `--work-rows`
and `--work-steps`. Structural engine caps, including the 64 MiB complete-image
ceiling, remain independent. Request files are limited to 16 MiB and 4096 rows,
with at most 256 fields per INSERT row;
UPDATE allows at most 4096 assignments. DELETE raw records are limited to 4096
bytes. `--value-bytes` defaults to 1024 input bytes and can be raised up to the
request-file ceiling. Binary counts encoded hex digits; UPDATE counts escape
prefixes too.

The mixed protocol preserves the written KIND even for NULL inputs. It validates
one-table/one-variable scope and flags before serializing native groups, prepays
its grouping name comparisons, and moves fixed/variable cells without cloning
Text/Binary payloads. The bounded variable/LVAL dispatcher adds Steps for its
classification pass; overall previous command-cost identity is not claimed.
The shared legacy parser remains the complete typed-value authority, and native
kind, request-count and source constraints remain enforced by the selected engine.

After complete preparation and bounded report formatting, the CLI uses the shared
[host publisher](../../../apps/tablecraft-cli/src/native/publication.rs), also
consumed by normal Jet 4 [query update](QUERY_EDITING.md#fresh-copy-cli) and the
explicit [SQL engineering-copy command](SQL_ENGINEERING_COPIES.md). The publisher
charges the complete requested output Bytes once before creating any temporary.
It writes a host-owned temporary in the output directory, syncs its file, and uses
`persist_noclobber` to publish a fresh final name. Existing files, source aliases
and destination symlinks are never overwritten. Failed write/sync/publication
cleans only the owned temporary, never an arbitrary final path. The publication
helper checks its cancellation callback before final publication; a cancelled
publication leaves no final copy. This does not install an OS signal handler.
This supplies no directory-entry
power-loss guarantee, durable transaction, in-place editing or automatic recovery.
A later stdout failure can leave the already completed copy in place.

Seven passing shared publication unit functions cover complete copies, injected
write/sync/publication failures, existing files and symlink/hardlink aliases,
final-callback cancellation, and exact versus one-below output-byte quotas. These
and two normal Jet 4 host controls pass within the complete 28-function CLI unit
run. The earlier workflow results below remain historical results for the original
owner before extraction; the four existing normal CLI update integration functions
are pending the current regression run.

`tablecraft_access::engineering` exposes the same typed preparation for other
hosts without opening or publishing files. `EngineeringCopy` has no journal,
commit or conversion to `NativeEditPlan`. [Existing native transaction work](NATIVE_TRANSACTIONS.md)
remains the separate qualified-plan boundary.

Ten focused CLI unit functions and five integration functions pass. They cover
actual UPDATE/INSERT/DELETE commands, quoted and typed inputs, source preservation,
no-clobber publication, output-byte accounting and failure cleanup. Initial
fixture/setup failures are retained in the
[verification record](../../testing/results/ace16-engineering-write-workflow-2026-10-29.json).
No Microsoft Access execution or native save comparison is claimed by these commands.

## Typed mixed DATA-placement validation

Before any new typed CLI action, the source gate qualified all 36 original
source/expected pairs through the actual shared pure constructor. There are no
new fixture copies, constructor changes or expected-image repairs. The first
source compile warned about an unused Value import in the new serializer; only
that import was removed before the API run, retaining the warning log/freeze.

All three focused serializer tests passed first: exact caller expectations,
original ordinals, KIND-based NULL classification, scope/category refusals,
all three cumulative exact pools, one-below limits and final cancellation.
All four integration functions passed first, including 36 complete typed copies
across both headers, directions and all three DATA destinations. Twelve use
non-NULL Binary hexadecimal input. Stale expected values/ordinals publish no
output, existing output bytes and original sources remain unchanged, and six
invalid-profile cases refuse before opening a missing source. Current CLI
regressions passed 34 unit and 25 integration functions: nineteen in the shared
SQL/typed UPDATE source target and six existing typed write workflows. CLI strict
all-target/all-feature checks passed. This is the selected consumer regression,
not a full workspace run. No Microsoft Access application write, normal-save or
durable-transaction qualification is claimed.
