# Captured clock defaults in ACE 16 INSERT previews

The explicit omission previews can resolve needed `Date()` and `Now()` defaults
using one caller-authorized operation clock. They return separate engineering
candidates; they do not enable normal ACE saves or journal admission.

Microsoft explicitly shows `Date()` as a Date/Time field default. Its field
DefaultValue contract permits expressions, and its Now reference describes a
current date/time result. These contracts support the default forms. They do not
establish native Access rounding or once-per-operation timing for this preview.
[Date default example](https://support.microsoft.com/en-us/access/introduction-to-expressions),
[Field.DefaultValue](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-defaultvalue-property-dao),
[Now function](https://support.microsoft.com/en-us/access/functions/now-function)

## Explicit operation input

`NativeImage::preview_ace16_insert_rows_with_omissions_and_clock` takes the
original `OmittedRowInsert` requests and an explicit expression `Clock` provider.
`preview_ace16_incrementing_insert_rows_with_omissions_and_clock` additionally
takes the existing exact `IncrementingAutoNumber` LAST/+1 policy. The generated
primary remains source-bound and separate from ordinary omissions; stored
NewValues mode is not inferred.

The source is fully qualified before default binding. Before reading the
provider, preparation checks every request width, every needed default shape and
native target, the actual record profile, and any generated LAST precondition
and requested range. Stale LAST and Int32 exhaustion therefore fail with zero
clock reads. Only unqualified zero-argument `Date()` and `Now()` calls on legacy
DATE fields are admitted. Parsing never invokes a general expression evaluator.

If any field needs a clock default, the coordinator reads the provider exactly
once. Every needed Date/Now in every requested row uses that immutable reading.
If no clock is needed, the provider is never read. Supplied values and supplied
NULL remain exact, including beside a needed clock default. Omitted literal NULL
still resolves without reading the clock. Empty operations also make zero reads.
Unknown needed calls refuse before capture; unknown unneeded defaults stay inert.

The projection reuses the existing expression `ClockReading` contract. Date is
midnight; Now uses the supplied civil time at whole-second precision. Fractions
are truncated by that contract. No timezone, instant or 100-nanosecond stored
DATE precision is inferred. This once-captured operation policy is explicit
engineering behavior; native Access statement timing remains unqualified.

The original literal-only omission methods retain their previous paths and
needed-clock refusal. The complete RowInsert/AutoNumberRowInsert APIs, normal
writes, slot/cascade policies and macro gates are unchanged. Time, Timer,
qualified calls, arguments, other runtime functions, UDFs and VBA remain outside
this default profile. Extended Date/Time defaults need separate qualification.

## Independent authority and failures

The captured reading stays local to the public operation coordinator and is
passed separately to materialization and the final-image audit. Neither
`Prepared` nor `Staged` contains trusted clock authority. The audit independently
rebinds original presence, source declarations and generation policy, then
projects from the original immutable reading. It never calls the host provider.
The shared projection reads only the pure-copy ClockReading implementation.

Clock cancellation, allocation and resource failures retain existing inspection
error categories. Other callback/projection errors become explicit
`EditError::DefaultEvaluation(EvalError)` failures. There is no blanket conversion
or retry. A later index, validation, cancellation or budget failure can follow a
single successful clock read; it still returns no candidate. Every source byte
outside the independently proved insertion effects remains protected.

## Observed verification

All source and expected images were qualified before clock API execution. The
first source selector ran two functions covering 24 date/time images and refusal
sources. A separately selected third function qualified four supplied/no-op
cases. A fourth source function qualified both MAX-counter sources. A fifth
qualified eight mixed supplied-NULL and literal-NULL images. None failed; the
split selectors were not retries.

The first normal all-features library check passed. The first clock API target
passed all 15 functions: five source, eight API and two causal audit controls.
All 36 complete expected cases match across both ACE 16 layouts and ordinary/
incrementing requests. Inputs include the year-0100 and year-9999 bounds, leap
noon, pre-epoch and epoch six o'clock, a midnight boundary and fractional civil
input. Old NULL payloads and supplied DATE alias bits are preserved; new NULL
cells use the existing record encoder's canonical bytes.

Counting providers prove one host read for needed defaults, zero reads for
supplied/no-op/literal cases and preflight refusals, and no provider replay by the
audit. Callback errors, all three budget pools, early and measured terminal
cancellation pass. Each causal audit control first matches a complete healthy
image, then rejects a correlated alternate reading/row/plan/candidate or poisoned
materialized field against the original operation input. No expected image was
repaired after producer execution.

The broader round passes 331 INSERT library functions, 55 Access library
functions and 88 CLI functions (37 unit and 51 integration). The first strict
Clippy run stops on one collapsible-if style finding in the shared CLI parser;
that parser-only correction is followed by a passing strict run. Minimal ACE and
Access builds pass. No clock engine or expected-image correction was needed.

The [Access/CLI consumer](../access/writes/SQL_INSERT_COPIES.md) has a separate source/API record.
No original-file public-writer comparison, native Access open/save, macro
execution or Windows timing claim is established by these literal images.

See the [literal omission contract](ACE16_OMITTED_INSERT_DEFAULTS.md) and
[focused result](results/ace16-clock-insert-defaults-2026-10-30.json).
