# Snapshot-local DELETE request capture

With `tablecraft-ace/ace16-write-preview`,
`NativeImage::prepare_ace16_row_deletes(table, ordinals, budget, cancelled)`
captures exact original record expectations for explicitly selected live-row
ordinals. It returns existing `RowDelete` requests in source order. The caller
can pass those requests to the existing DELETE preview on the same image.

Capture checks the ordinary engineering delete profile, table admission,
duplicate and missing ordinals, resource limits and terminal cancellation.
Record headers, variable data and NULL masks remain raw bytes; values are not
decoded, coerced or reconstructed by a host. Ordinals are snapshot-local and
must not be treated as persistent identities. The helper does not modify a
source or grant normal save, macro, journal or transaction permission.

The source-only qualification function passes before helper execution. All five
focused functions pass, covering exact NULL/empty/Binary records, source order,
same-image deletion, stale expectations, duplicate/missing ordinals, empty
admission, cumulative resource pools and cancellation at the final callback.
The [verification record](results/ace16-ordinal-delete-selection-2026-10-29.json)
retains code/input pins and logs. These are local engineering checks, not native
Access comparisons.
