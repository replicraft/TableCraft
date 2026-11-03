# Wider numeric key updates with ordinary fields

Normal fixed UPDATE now admits qualified Int64/Currency indexed tables with
retained ordinary scalar fields. Numeric width and manual fixed-field checks
apply to every actual indexed key and named indexed target, rather than every
unrelated column. Dense native numbering, complete canonical Source, constraints,
properties, required-program checks, alias ownership and LVAL exclusion remain
mandatory. The retained-NODE route keeps its existing numeric-only schema rules.

Independent ACE 16-family header-five/six inputs contain Int64Companions and
CurrencyCompanions, each with Id, KeyValue, Marker, Enabled, Label and Opaque.
The qualified updates change KeyValue while preserving Boolean values, Text,
Binary payload/empty/NULL, variable-record framing and every unrelated byte.
All physical indexes are qualified single-column numeric root leaves. This does
not establish nonnumeric indexed-target or mixed-NODE mutation support.

The existing record stage, fitting numeric kernel and independent whole-image
proof remain the mutation owners. NULL assignment retains inactive fixed bytes;
NULL-to-present and atomic endpoint swaps rebuild complete final populations
using original ROW ties. Each physical index is handled once across its aliases.
Original prefix zero, historical counters, roots, maps, protected inactive tails,
the other table and EOF remain exact. Existing generation/default and
required-program policies remain in force.

Twenty-four original request contexts include MIN-to-zero, MAX-to-NULL,
NULL-to-MIN, atomic MIN/MAX swap, equal assignment and named-empty groups.
Only the two-row swaps have a second order: 28 native commits and typed saves
match complete independent files. The direct proof rejects 720 protected-byte
forgeries; checked original-intent journal recovery rejects another 720.
There are 408 recovery contexts, 112 committed mismatches refused without effects,
296 completion confirmations and 24 measured full public resource operations.
These observations follow [independent source qualification](../../testing/ACE16_WIDE_SIBLING_AFTER_SOURCES.md).

Typed edits preserve exact Int64 and Currency carriers. SQL and CLI retain their
current checked assignment rules. The sixteen qualified SQL/CLI cases use
Int64 arithmetic `0 - 0`, Currency `CCur(0)`, NULL, self-assignment or an empty
selection; extrema restoration and swaps have typed/native coverage. Small
integer literals were outside that initial save matrix. The shared
[SQL assignment continuation](../../query/QUERY_EXECUTION.md#sql-stored-assignment)
now accepts checked integral widening/scaling, with four additional plain-zero
CLI saves against the same independent images.
For a table satisfying the profile, an explicit offline command is:

```sh
tablecraft query journal-update \
  'UPDATE [Int64Companions] SET KeyValue = 0 WHERE Id = 1' \
  --database example.accdb --journal example.journal
```

The shared original TCNI4 intent, checked V1 replay and retained typed/SQL/CLI
services are unchanged. See the [acceptance record](../../testing/results/ace16-wide-sibling-writes-2026-11-02.json)
for first failures, corrections and exact verification scope. Independent
readers and portable tests do not establish native Access save/reopen. General
allocation, ignored-NULL keys, broader index families, shared-file writes and
native Access qualification remain separate work.
