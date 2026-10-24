# Bounded numeric means

Read-only `Avg` and `DAvg` now share a dedicated mean accumulator. Currency
averaging no longer enters the unsupported legacy Currency division path, and a
mean can succeed when its intermediate integral/Currency sum exceeds the source
carrier. Source values and stored schemas are unchanged. Floating Sum/DSum now
share the [bounded binary accumulator](testing/FLOATING_SUMS.md); fixed-point sums
retain their separate checked fold.

NULL inputs do not contribute to the total or count. Empty/all-NULL selections
return NULL. The executor still evaluates each selected operand once, checks
cancellation/work for each row and enforces its existing row/source/output limits.
The accumulator retains one bounded numeric total, not a second record buffer.

## Current result and precision profile

These rules apply to values actually delivered to the accumulator. A
[UNION result conversion](UNION_TYPES.md#provisional-mixed-numeric-profile) happens
first: a Decimal/Double UNION supplies Double rows and has a Double mean. Dynamic
conditional values can still deliver both original carriers and retain the
Decimal precedence below. Source values and accumulator kernels are unchanged.

| Observed non-NULL inputs | Accumulation and result |
| --- | --- |
| Ordinary integral/Boolean values | Bounded exact decimal total; Double mean |
| Currency, optionally mixed with ordinary integers | Bounded exact decimal total; Double mean |
| Single/Double or legacy DATE serials without explicit Decimal | Bounded exact binary total once floating input appears; one rounded Double mean |
| Explicit Decimal, including mixed numeric values | Exact total with wider fallback; bounded Decimal mean |

The existing scalar conversion, Decimal arithmetic and bounded rational services
own these operations. Decimal accumulation and Decimal-valued quotients use
rust_decimal; non-Decimal means now round their exact rational total/count
directly to Double, including totals that fit the Decimal carrier. When an exact sum cannot fit it,
a bounded i128/BigInt fallback aligns coefficients without discarding digits.
num-integer supplies quotient/remainder; final wide Decimal quotients use
half-even rounding at the highest scale up to 28 whose coefficient fits 96 bits. Explicit Decimal
retains precedence. Conversion of a previously floating total to Decimal follows
the existing normalized conversion policy. Retained scale is representational:
`1.8750` and `1.875` are not interchangeable core values, even when numerically equal.

Non-Decimal successful means select Double, including Currency means; conversion
can round to binary64. Large integral means no longer require their intermediate
sum to fit Int64 or to be exactly representable as Double. This is the mean's
explicit result conversion, not a relaxation of integer operators, parameter
carriers or stored fields. Returned metadata describes the qualified carrier;
runtime domain targets with unknown expression types can remain Dynamic.

Other nonnumeric fields do not acquire implicit numeric/date interpretation. Operational
failures remain failures, including late invalid operands. No failed operand
becomes NULL. Decimal-valued quotients whose totals fit Decimal reuse the checked
promoted-division service.

## Limits and remaining compatibility work

Decimal totals can exceed the stored 96-bit envelope: scale alignment and the
checked i64 count bound the fallback coefficient to at most 254 bits. BigInt
scratch is charged before allocation; work and cancellation checks remain explicit.
Results still use the validated 96-bit, scale 0–28 Decimal carrier. Ordinary
rust_decimal quotient behavior, retained scales and conversions remain unchanged.
[Exact binary floating means](testing/FLOATING_MEANS.md) avoid floating-total
overflow and preserve cancellation residuals. Mixed-type conversion order, full
Variant/SQL result typing and native ACE/Jet rounding/precision qualification
remain open. In particular, a floating prefix converted after Decimal appears
still passes through the current bounded scalar conversion policy.
This is a working current profile, not complete native Access aggregate behavior.

Microsoft's [Avg reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/avg-function-microsoft-access-sql)
defines the mean of numeric data with NULL fields excluded, and includes a freight
average example. It does not qualify our precise accumulator/carrier choices.
Windows Access comparisons remain separate from the independent evidence below.

## Evidence

Rust controls reproduce the former UnsupportedNumericCombination and intermediate
Currency overflow failures, then check ordinary/domain means, NULL/empty input,
Currency extremes/cancellation, wider integral totals and mixed Decimal precedence.
Synthetic ACE 16-family native fixtures check Currency data and ordinary/domain
queries with metadata-only binding. The original
[Fraction/CLI runner](testing/compare_currency_average.py) checks 69 exact rational
integer/quarter-unit cases, including values that overflow a Currency sum. It uses
source-free UNION inputs and Python Fraction, not a native Access SQL engine.


[Exact floating means and measured costs](testing/FLOATING_MEANS.md) extend this
profile for pure floating inputs; bounded mixed-type/Decimal and native behavior
remain distinct.

## Wider Decimal totals (22 October 2026)

Focused Avg/DAvg controls cover repeated maximum coefficients at scales 0 and 28,
negative extremes, mixed-scale cancellation down to 1e-28 and positive/negative
halfway rounding. These are independent mathematical controls, not Access results.
The dedicated `decimal_averages` release benchmark includes result destruction and
excludes setup/binding; its ordinary cases are kept separate from overflow goldens.

Six alternating release pairs on this local Darwin/arm64 host measured median
execution of 4,096 rows/one group at 0.910 ms before and 0.939 ms after (about
29 microseconds, 3.2% more). For 2,048 rows/128 groups, medians were 3.109 and
3.097 ms, within the observed spread. No broad speed improvement is claimed.
[Raw samples](testing/results/decimal-means-2026-10-22.json) retain the initial
single runs as well as every paired sample; baseline is `233cd29` with the identical
benchmark harness. Run `cargo bench -p tablecraft-query --bench decimal_averages`.


## Native adapter and independent rational controls

The native adapter regression reuses original synthetic NUMERIC/index fixture
recipes for profile bytes 1, 2, 5 and 6. Each reads nine positive or negative
28-digit coefficients, plus a NULL, at declared scale 0 or 28. Their exact totals
exceed the core carrier. Avg and DAvg return the original coefficient/scale;
preparation reads no user records and retains Decimal result metadata. This
qualifies the tested engine-to-query path, not Microsoft Access SQL behavior.

The original [Fraction/CLI Decimal runner](testing/compare_decimal_average.py)
passes 123 independent controls, including full core maxima, nonterminating
quotients, positive/negative halfway cases, mixed scales, tiny cancellation and
96 deterministic generated groups. Python Fraction computes exact means and
rounds each candidate scale independently with ties to even. It compares numeric
values; retained-scale representation has separate Rust goldens. Run:

```sh
python3 docs/testing/compare_decimal_average.py --cli target/debug/tablecraft
```

No third-party fixture/code or native Access result is introduced. These checks
do not qualify mixed-client behavior, other native field encodings or writes.

## Single rounding for fixed-point Double means (23 October 2026)

A fixed-point mean previously divided its narrow total into a scale-28 Decimal,
then converted that quotient to Double. That introduced double rounding. One
Currency input of `0.0005` and 16,912 zero inputs reproduces it: the exact rational
is `5 / (10000 * 16913)`, whose nearest binary64 has bits
`0x3e5fbe3b440519ce`. The intermediate Decimal quotient instead converted to
`0x3e5fbe3b440519cf`. Both Avg and DAvg now use the existing bounded rational
finalizer for integral/Boolean/Currency means, avoiding that intermediate quotient.
Negative inputs have the corresponding signed result. The Rust executor control
failed before the fix and passes afterward. Explicit Decimal means still retain
their Decimal quotient, scale and rounding policy.

The finalizer charges its existing bounded BigInt/rational scratch before
allocation and checks work/cancellation. There is no second row pass, provider
call, dependency or custom decimal implementation. Mixed-type encounter order,
full Variant/result declaration rules and native Access precision remain open.
Microsoft's Avg reference defines the arithmetic mean and NULL exclusion; it
does not qualify this final rounding policy.

The extended original `compare_currency_average.py` runner passes 125 independent
controls: 101 source-free cases, including odd population counts, plus 24 native
queries over the registered Jet 4/ACE 2007/2016/2019 inputs. Reviewed inert
Jackcess getters supply scaled Currency inputs; Python Fraction computes exact
means. Positive and negative 16,913-row regressions agree in sequential and both
ID-index orders, through Avg and DAvg. The runner checks JAR/input hashes and
unchanged original bytes. Native Microsoft Access remains unrun.

```sh
python3 docs/testing/compare_currency_average.py \
  --cli target/debug/tablecraft --jar path/to/reviewed/jackcess-5.0.4.jar \
  --native-directory path/to/registered-large-indexes
```

The [expanded averages harness](../crates/query/tablecraft-query/benches/averages.rs)
now includes Currency and Int64, each with one group and 128 groups, alongside
its floating controls. Both revisions use this identical harness; baseline is
`4f8e6a1`. Six alternating release pairs measured Currency one-group medians
of 1.050 versus 1.054 ms and 128-group medians of 3.233 versus 3.233 ms.
Integral medians were 1.048 versus 1.043 ms and 3.233 versus 3.232 ms. Floating
controls were 0.912 versus 0.911 ms and 3.287 versus 3.347 ms. There is no clear
overall regression or speed improvement; the unrelated many-group floating
control varied by about 60 microseconds. Hyperfine whole-process means were
274.0 ± 3.3 versus 273.1 ± 2.9 ms over seven runs after two warm-ups.
[Raw evidence](testing/results/fixed-mean-rounding-2026-10-23.json) retains every
sample and harness/executable hashes, with no exclusions. Timings include result
destruction, exclude setup/binding from per-query results and exclude native I/O.


## Legacy Date/Time means

Avg/DAvg now admit legacy DateTime values through their stored Automation DATE
serials. NULL values are omitted before conversion, so direct Avg(Stamp) can
average nullable Date fields without invoking CDbl(NULL). Empty/all-NULL selections
return NULL. The existing exact binary accumulator supplies one rounded Double
mean; Avg over a declared Date field exposes nullable Double result metadata
without reading rows during preparation. Explicit Decimal in a heterogeneous
expression retains the existing Decimal-result profile. Date/Time Extended
remains unsupported as an aggregate numeric operand. Mixed numeric inputs keep
the existing prefix conversions and encounter-dependent Decimal bridging rules.

Microsoft's [query aggregate table](https://support.microsoft.com/en-us/access/sum-data-by-using-a-query)
explicitly includes Date/Time for averages. It does not specify the precise
return carrier, negative-serial treatment, mixed-family promotion or accumulation
precision. The Double-serial result described here is TableCraft's engineering
profile, not a native Access result-type claim. Native comparison of Avg/DAvg
Date results remains pending. Existing Date Sum refusal and statistical numeric
admission remain separate contracts.

The calculation uses raw serial arithmetic, not elapsed civil time. For example,
-0.25 and +0.25 can denote the same calendar time under Automation's
negative-fraction interpretation, yet their serial mean is zero. No intermediate
civil rounding, timezone lookup or calendar-format policy runs. Call CDate on a
representable resulting serial when a Date-valued expression is needed.

```sql
SELECT Avg(Stamp) AS MeanSerial, CDate(Avg(Stamp)) AS MeanDate FROM Events;
```

CDate(NULL) still fails; callers must handle an empty/all-NULL result explicitly
before requesting that conversion. Shared query allocations, work, cancellation,
clock ownership and buffered failure publication retain their existing limits.


Six focused date-mean Rust tests include 128 fixed-seed generated quarter-day
sequences, no-scan metadata, named/derived/grouped/crosstab consumers, negative
serial aliases, Decimal mixtures, one clock call per row and every observed
cancellation checkpoint. CLI controls exercise projection and atomic late failures.
The independent [DATE average runner](testing/compare_date_average.py) passes
144 controls: 72 source-free exact-rational cases and 72 native query cases on
unchanged registered four-family large-index inputs. Jackcess supplies inert
raw DATE bits; Python Fraction supplies expected means. Sequential and both
ID-index directions cover direct/domain means, derived results, grouping,
crosstabs, empty groups and mixed Date/Double inputs. Only TableCraft executes
SQL; native result-type and precision comparison remains pending.
