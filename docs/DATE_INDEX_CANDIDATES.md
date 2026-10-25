# Calendar-safe native DATE candidates

The engine now accepts `IndexProjectionSelection::DateTime` with a validated
`DateTimeIndexSelection` through the existing explicit index projection/request
APIs. This is working native-reader functionality, separate from SQL constraint
extraction or automatic index selection. The Access/query adapter does not yet
issue these requests.

The supported storage key is one qualified legacy eight-byte DATE column. All
retains complete traversal/count checks, Null selects only NULL keys, and
CalendarCandidates accepts included/excluded/unbounded `LegacyDateTime` endpoints.
A request remains type-qualified even for All/Null: Currency, Double, Int64,
compound and extended-date indexes cannot become a DATE scan through shared width.
Existing index eligibility, owner/map/row-key validation, direction, projections,
consumer ceilings, budgets, cancellation and selected-read failures remain active.

## Candidate coverage contract

Calendar order uses core's checked day/time comparison. Native index order uses
exact retained serial bits; these orders differ before the Automation epoch.
Negative/positive epoch fractions can also denote equal times. The
[Microsoft DATE contract](https://learn.microsoft.com/en-us/cpp/atl-mfc-shared/date-type?view=msvc-170)
and existing [core representation](CORE_TYPES.md) explain that distinction.

Inclusive equal endpoints select the exact physical serial for non-epoch days.
On epoch day, a span retains both signs, including both signed zeros; it can
also retain intermediate times. General ranges conservatively retain full
boundary calendar days. Negative boundary days reverse their fractional order,
and epoch day spans both signs. Reversed intervals and equal intervals with an
excluded endpoint select no candidates. Unbounded calendar ranges exclude NULL;
All and Null retain their separate meanings.

Candidates can include extra records. A caller must apply its complete temporal
predicate afterward; this API is not a replacement equality/range comparer.
Output retains original DATE bits without civil conversion, rounding, timezone
inference or canonicalization. Explicit index reads keep physical key order;
the existing table-order reader retains sequential root order. A broad candidate
span can exhaust its row/work allowance even when few final rows match; no hidden
sequential fallback, spill or complete-table cache is introduced.

## Current evidence

Five unit tests include a 512-case generated coverage property, endpoint matrices,
negative-time reversal, both epoch aliases, signed zero, tiny fractions and year
limits. Five native-reader tests exercise independent authored key/data bytes in
Jet 4 and ACE 2007/2016/2019 headers, both directions, exact residual results,
NULL, projected points under a one-row ceiling, table order, wrong-carrier
refusal, compound refusal, corrupt selected roots, hidden DATE-key payload
validation, byte exhaustion and cancellation. Nine existing
DATE/projection regressions pass. Strict engine linting with all features,
minimal adapter compilation and formatting pass; see the
[scoped verification record](testing/results/date-index-candidates-2026-10-25.json).

The first native-test compile used an incorrect error variant name; its excluded
log remains recorded, and only that assertion was corrected to the actual typed
Bytes-limit error. No production algorithm or expected result changed to make
these tests pass.

This does not establish native Microsoft Access SQL predicate equivalence, new
relationship/write eligibility or live-file coherence. Original-file candidate
replay and SQL/automatic source integration are separate next acceptance work;
existing complete DATE index evidence remains in [NATIVE_INDEXES.md](NATIVE_INDEXES.md).
