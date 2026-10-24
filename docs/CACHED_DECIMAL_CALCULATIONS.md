# Cached Decimal calculations

Explicit stored-calculation inspection and browsing now read the observed ACE
16-family calculated Numeric layout as exact Decimal values:

```sh
tablecraft inspect copy.accdb --table CalculatedDecimals --stored-calculation Amount
tablecraft rows copy.accdb CalculatedDecimals --stored-calculations --format tsv
```

These display the original cached result, including its retained fractional scale.
They do not evaluate the expression, assert freshness, update the database or make
calculated fields eligible for ordinary query execution. Existing row headers mark
calculated columns `[cached]`. The distinct row-local preview can inspect the same
Decimal declaration and separately return `Recomputed` values under its existing
expression rules; it does not coerce those results to the cached Decimal carrier.

## Observed physical profile

The existing calculated-field gates still require ACE 2016/2019-family profiles,
variable updatable storage, exact calculated flags, matching typed ResultType and
a nonempty Expression. The Numeric result additionally requires capacity 39 and
raw column precision/scale bytes `(0, 0)`, even on empty tables. Those zero bytes
are retained as physical declaration metadata; they are not an ordinary Decimal
column's `(precision, scale)` constraint.

After the checked outer calculation wrapper, the admitted payload is 16 bytes:
a little-endian 14-byte remainder count, effective scale, sign, and a 96-bit
magnitude. The magnitude is stored as a little-endian high 32-bit word followed
by a little-endian low 64-bit word. Canonical sign bytes are zero and `0x80`.
Effective scale is 0 through 28, and the magnitude must contain at most 28 decimal
digits. Decimal construction preserves the coefficient and scale without floating
conversion, normalization or rounding. For example raw column scale zero can
coexist with a cached value `-1.2345` or `0.0000`.

Absent storage and a qualified empty calculation wrapper retain the existing
NULL behavior. Present zero with scale four remains `0.0000`. Alternate widths,
headers, signs, scales, excessive magnitudes and negative-zero encodings fail
explicitly. Other calculated Numeric geometries, Long Text, extended dates and
error caches remain unqualified. No ordinary 17-byte Decimal codec is reused for
this different layout, and no new dependency is introduced.

Declaration inspection remains metadata-only, including over corrupt cached
payloads. Cached reads use the existing cumulative native controls, cancellation,
unchanged/offline source requirement and atomic complete-snapshot publication.
A malformed later value suppresses the entire buffered inspection. Ordinary pages
retain their documented partial-read validation range.

## Evidence and limits

The original [fixture and getter oracle](testing/CalculatedDecimalFixture.java)
creates fresh ACE 2016/2019 files with expression evaluation disabled, then opens
them read-only. Independent `BigDecimal` getters provide exact coefficients and
scales; public in-memory column serialization provides wrapper observations.
The reviewed Jackcess 5.0.4 artifact's `CalcNumericColImpl` is a physical reference,
not an adopted implementation. Microsoft's [calculated-field properties](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties)
document stored calculation results and Result Type, not these physical bytes.

The [comparison runner](testing/compare_calculated_decimals.py) checks 22 cached
cells across both files, retained zeros, signs, 28-digit extremes, scales through
28, empty declarations, cached row browsing, ordinary-query refusal and late
corruptions on disposable copies. The
[24 October 2026 report](testing/results/calculated-decimals-2026-10-24.json)
records 82 passing controls and unchanged fixture, oracle, implementation and
immutable executable hashes. This is independent cached-storage evidence;
Microsoft Access result admissibility, native recalculation and freshness remain
unqualified. Original databases are never modified.
