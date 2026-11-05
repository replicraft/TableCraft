# Datasheet totals

Totals are derived values for a complete native table view. They are separate
from stored rows, record selection, calculated-field caches and edit rights.
`tablecraft-runtime` owns admission, scanning, aggregation and typed display
facts; the desktop forwards requests and native filter capabilities. Shared UI
controls do not calculate over the currently displayed page or parse cell text.

## Native service

`DescribeTotals` reads field metadata without decoding user records. The
description lists eligible operations for each field. `Totals` accepts explicit
column/operation pairs and a target containing the document revision, table,
filter identifier and enabled state. A filtered request also needs the exact
host-owned native handle; a target alone grants no source access.

| Field | Current operations |
| --- | --- |
| Ordinary numeric, including Currency | Count, Sum, Average, Minimum, Maximum, sample Standard Deviation and Variance |
| Legacy Date/Time | Count, Average, Minimum and Maximum |
| Extended Date/Time | Count, Minimum and Maximum; Average remains unsupported by the query kernel |
| Supported Text, Boolean, Binary and GUID carriers | Count |
| Unsupported complex/calculated physical carriers | No aggregate offered |

Count excludes NULL and includes empty text. Empty input produces Count 0;
other aggregates retain the existing query engine's empty-set rules. Sample
statistics require at least two non-NULL values. The service composes an owned
aggregate syntax model and reuses the query engine. It neither interpolates
identifiers into SQL nor implements another arithmetic accumulator.

The native cursor reaches checked EOF before publication, even for an empty
operation list. Filter membership is matched against original source ordinals,
independently of display sorting. The final source and selected counts must
agree with the retained handle. Failed or cancelled requests publish no prefix
and do not modify the old handle or records. Totals describe the captured source;
they do not reopen a file or silently consult newer external writes.

The current service allows 32 distinct selected fields. It retains one bounded
projection for the aggregate plan: native reads retain their existing 100,000-row
ceiling, while the query plan retains its independent 10,000-row ceiling. Retained
projection, query, native comparison and output pools remain separate. Exceeding
a limit refuses the result rather than substituting visible-page totals.

## Derived display

Fixed Currency Sum/Minimum/Maximum retain their typed result. Currency Average
retains the query's Double result with a distinct derived display fact. Formatting
applies the selected currency's minor-unit rounding once; it does not pass through
a four-place stored Currency conversion. Raw exposes the original Double spelling.

Legacy Date/Time Average retains an explicit checked Automation-date display
projection. Recognized source date/time formats apply to the derived date value.
Count and statistics do not inherit checkbox controls or stored-calculation
provenance. Display choices never change aggregate inputs or source values.

## Shared interaction

Home Totals toggles the row. Initial fields are None; enabling the row requests
metadata, and choosing an aggregate explicitly requests computation. Each field,
including the first, has its own eligible menu. A retained unsupported choice
still offers None so it can be removed. Exceeding the 32-field ceiling shows a
refusal and preserves the existing choices.

Visibility and choices belong to the table object. Same-view page changes reuse
complete results; filter/view changes invalidate them. After reviewed save/reopen,
only source-free preferences survive, followed by fresh metadata and computation.
Region changes reformat typed results, including parked objects, without scanning
or parsing displayed text. Errors stop automatic retries; hiding a failed or
pending result discards it before a later explicit show can request a fresh result.

A selected total is a separate, source-bound selection owner. Keyboard Copy from
the focused total and Home Copy use its complete displayed value. Paste, record
deletion and native cell editing cannot fall through to a previously selected
record. Row-specific Find,
Zoom and selection filters likewise require a native selection. Returning focus
to a native cell restores those commands without changing its original identity.

The row follows records/New when they fit; it stays reachable beneath the body
when records overflow. It shares the actual resized/scrolled header rectangles.
This adaptive overflow placement is a presentation approximation, not a recorded
native Access interaction. Totals are not inserted into `Data`, record numbering,
row maps or navigation.

## References and qualification

Microsoft's [Totals-row guide](https://support.microsoft.com/en-us/access/display-column-totals-in-a-datasheet-using-a-totals-row)
documents field-dependent choices. The viewed images show an initially unset row,
with Total in the first field, below New; the first field can subsequently show
an aggregate. The [visual reference record](VISUAL_REFERENCES.md) distinguishes
these observations from unknown long-scroll behavior and native qualification.
Microsoft's [Access SQL Count contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
establishes NULL exclusion independently of VBA execution.

Computing over current filtered membership is the implementation's interpretation
of a current-view total. It has independent fixture coverage, but no recorded
native Access comparison yet. Query-result totals, persistent Access totals
preferences and Extended Date/Time Average remain unfinished. No write capability
or broader file compatibility follows from aggregate or UI tests.


## Local verification — 5 November 2026

Six dedicated runtime cases cover complete views beyond 100 rows, all seven
numeric operations, filter modes and stale handles, NULL versus empty text,
date/currency lineage, excluded NaN, failure and cancellation. The complete
runtime/display batch passed 101 runtime tests, seven display tests and one
runtime diagnostics test; minimal strict Clippy passed for both libraries.

The desktop suite passed 56 cases before the final save/reopen addition. All four
Totals desktop cases then passed, establishing 57 distinct desktop cases across
the batches. The native save case changes Score 20 to 25 through actual
Review/Apply, acknowledges a changed-row receipt, reopens a fresh revision and
observes a total of 65 rather than 60. It also refuses the old revision's targets.

The final UI gate passed 197 functional checks plus the separate full preview
matrix: 63 scenes × eight locales × two themes (1,008 combinations), for 198 UI
checks. Focused Totals cases cover source ownership and refusal, no-record
geometry, the 32-field boundary, accessible toggle state, same-view page reuse,
parked formatting, failed-hide refresh and cancellation. Strict eight-package
Clippy and repository format/documentation/dependency checks passed.

Four German/de-DE light/dark Totals and Totals-error PNGs were viewed from ignored
`.cache/ui-totals-review/`, rendered on macOS with Windows keyboard conventions
at 1000 × 680 logical pixels, scale 1.25 (1250 × 850 physical pixels). The final
error copy was re-exported after removing duplicate mock-status text. These are
windowless presentation checks, not native Access or Windows/Linux qualification.
