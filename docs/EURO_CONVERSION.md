# EuroConvert query profile

EuroConvert now executes in scalar/bound queries and over explicit native table sources
through the shared financial call path. It returns Double and does not apply a
number format, read live exchange rates, discover a locale or execute stored VBA.
This advances the [built-in tracker](ACCESS_BUILTINS.md) from Not yet to Partial.

## References and accepted envelope

The [Access function reference](https://support.microsoft.com/en-gb/access/functions/euroconvert-function)
establishes expression/field use and optional arguments. The
[Access method contract](https://learn.microsoft.com/en-us/office/vba/api/access.application.euroconvert)
specifies Double input/result, target calculation precision, same-code identity
and significant-digit triangulation. Exact conversion factors come from the
[ECB's irrevocable rates](https://www.ecb.europa.eu/euro/intro/html/index.en.html),
not Microsoft's obsolete illustrative rates. These are format-compatibility
references; they do not replace native Access function evidence.

| Code | National units per EUR | Target calculation places |
| --- | --- | --- |
| ATS | 13.7603 | 2 |
| BEF | 40.3399 | 0 |
| DEM | 1.95583 | 2 |
| ESP | 166.386 | 0 |
| FIM | 5.94573 | 2 |
| FRF | 6.55957 | 2 |
| IEP | 0.787564 | 2 |
| ITL | 1936.27 | 0 |
| LUF | 40.3399 | 0 |
| NLG | 2.20371 | 2 |
| PTE | 200.482 | 0 |
| EUR | 1 | 2 |

This is Microsoft's main Access currency table. Its later-version appendix
mixes a subsequently participating currency with nonparticipants and provides
neither complete factors nor rounding rules. Later participant codes, including
GRD and those in newer ECB tables, remain unqualified rather than silently
assuming installed Access versions accept them. GBP, DKK, SEK and unknown codes
are rejected; no market FX quote is substituted. Codes are exactly three ASCII
bytes and compare without ASCII case; surrounding spaces are not trimmed.

```text
EuroConvert(number, source, target [, fullprecision [, triangulationprecision]])
```

The amount uses the existing finite numeric-to-Double conversion. Currency codes
must be Text. Full precision defaults to False and uses existing numeric Boolean
conversion when supplied. Triangulation precision, when present, uses the checked
16-bit Integer conversion and must be at least three. Explicit NULL is an error,
separate from an omitted slot. Numeric Text uses explicit-policy contextual
conversion for these numeric slots. Full Variant admission and native coercion
remain unfinished; no ambient separator policy is inferred.

Trailing omitted optional slots are removed before evaluation. An omitted full
precision slot before an explicit triangulation count becomes False. Required
omissions, wildcards and excess arguments fail before operand execution; valid
calls evaluate every supplied expression in written order, including arguments
that a same-currency result would not need.

After argument validation, equal codes return the converted original Double
without currency rounding. Otherwise the amount is divided by the source factor,
then multiplied by the target factor. Full precision suppresses final rounding;
it does not suppress an explicitly requested intermediate rounding. The omitted
triangulation count leaves the intermediate unrounded. An explicit count rounds
the intermediate euro amount to that many significant digits only when both
endpoints are national currencies. Seventeen or more digits leave Double unchanged.

Final rounding uses the target calculation places with a midpoint-away policy.
Large integral-magnitude Doubles bypass unnecessary multiply/round/divide work.
Intermediate significant rounding splits scaling for subnormals instead of
constructing overflowing powers. Nonfinite input, overflow and unintended nonzero
intermediate/final underflow in full-precision mode fail explicitly. No Decimal
storage ceiling is imposed on finite Double identity conversions.

The significant-digit interpretation follows Microsoft's Access parameter text.
Legal monetary rounding rules and spreadsheet implementations must not be treated
as an Access oracle: native intermediate precision, omitted-count policy,
midpoints, error order and code acceptance still require comparison.

## CLI examples

```sh
tablecraft query eval "SELECT EuroConvert(1.2,'DEM','EUR')"
# 0.61
tablecraft query run "SELECT EuroConvert(1000,'FRF','DEM',True,3)"
# 297.28616 under the explicit significant-digit intermediate policy
tablecraft query run "SELECT EuroConvert([Money],'DEM','EUR') FROM ScalarSamples WHERE [Money] Is Not Null" --database example.accdb
```

The library owns the calculation and call normalization. CLI, native adapter and
query binding add no duplicate exchange-rate logic. Rate lookup and operation
boundaries retain shared work/cancellation checks; the kernel allocates no output
strings or currency tables. Existing financial functions retain their defaults.
No library dependency or stored numeric carrier changed.

## Evidence and remaining qualification

Four kernel controls cover each rate/target precision, identity, code matching,
triangulation, finite extremes, invalid parameters and a fixed-seed 256-case
identity model. Two scalar controls cover optional slots, eager errors, work/byte
limits and every observed cancellation checkpoint. Two bound controls cover
field/group/aggregate/correlated dependencies and currency/precision plan reuse.
CLI controls exercise both query commands and empty stdout on failure.

The original [independent runner](testing/compare_euro.py) passes 4646 controls:
all 144 currency pairs with four input amounts and four precision policies in
both commands, optional/refusal probes, and Currency/NULL-field retrieval over
four original ScalarFixture MDB/ACCDB files. Getter expression evaluation is
disabled and original hashes remain unchanged. Decimal models compare Double
outputs with an eight-ULP or 1e-12 absolute tolerance, whichever is larger; this
is numerical evidence, not bit-for-bit Access equivalence. No new binary fixture
or third-party source was copied.

Windows Access results, exact intermediate/rounding/error/coercion equivalence,
later participant profiles and complete native saved-query qualification remain
open. EuroConvert remains Partial and does not complete the wider read/query
acceptance roadmap.


Numeric Text amount/full-precision/triangulation inputs now use the existing
explicit number-format conversion, in the kernel's original validation order.
Currency codes are not numerically converted. Typed options retain Boolean and
Integer rounding/range rules; native coercion/rounding remains unqualified.
See [shared financial context](FINANCIAL_FUNCTIONS.md#contextual-numeric-text-inputs-23-october-2026).


EuroConvert now shares [ordered financial expression-state admission](FINANCIAL_FUNCTIONS.md#ordered-expression-state-admission-23-october-2026).
All supplied SQL expressions evaluate eagerly before amount/currency/option
validation. Error data fails TypeMismatch at its requested position, preserving
an earlier unknown-currency Domain failure. Host Empty numeric controls use zero
under the existing targets; Empty currency codes remain refused. No fixed rates,
rounding, option defaults or native qualification claims change.

The public retained-value API now also accepts active String ranges in currency
code and numeric/Boolean/Integer option positions, using
[staged financial admission](FINANCIAL_FUNCTIONS.md#retained-string-evaluator-parity).
It publishes each reached range strictly, then reuses the existing parser or
converter. Source/target validation still precedes option admission; codes do
not gain numeric conversion or a NumberFormat requirement. Ordinary supported
query results and native qualification limits are unchanged.
