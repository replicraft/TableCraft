# Scalar depreciation functions

`SLN`, `SYD` and `DDB` extend the closed [financial function profile](FINANCIAL_FUNCTIONS.md).
They return Double after the existing numeric/Boolean conversion, without
Currency rounding, text parsing or external financial/VBA execution. Explicit
NULL and nonnumeric/non-finite carriers fail. Both SQL evaluators eagerly evaluate
all supplied expressions before conversion and share required/optional slot rules.

| Function | Required arguments | Optional argument |
| --- | --- | --- |
| SLN | Initial cost, salvage, useful life | None |
| SYD | Initial cost, salvage, useful life, selected period | None |
| DDB | Initial cost, salvage, useful life, selected period | Declining factor; default 2 |

```sh
tablecraft query eval "SELECT SLN(1200,200,4) AS Straight,SYD(1200,200,4,1) AS Weighted"
tablecraft query run "SELECT DDB(1200,200,4,1,) AS Declining"
```

SLN has exactly three arguments; SYD exactly four; DDB four or five. Only DDB's
optional factor may be omitted, including a trailing missing slot. Its default is
two, independently of the zero defaults for annuities. Explicit NULL/zero factors
are not replaced. Wildcards, missing required slots and additional arguments fail.
All source/query budgets, cancellation checkpoints and buffered publication
boundaries remain active. UPDATE's separate pure profile is unchanged.

## Calculations and current reference profile

SLN divides cost minus salvage by life, rejecting zero life. Signed finite inputs
and negative life currently follow the algebraic Microsoft financial-runtime
profile. When subtraction overflows but the quotient is representable, a scaled
path avoids that intermediate overflow.

SYD weights the depreciable basis by the remaining year digit divided by the
sum of all life digits. It requires nonnegative salvage and `0 < period <= life`.
Negative cost currently retains the signed result. Division is ordered to avoid
overflowing `life*(life+1)`; subtracting period before adding one retains the final
digit at very large finite life values. Fractional life/period inputs are not
silently rounded to Integer or prorated.

For ordinary DDB schedules, each charge is the smaller of the declining fraction
of the preceding book value and its remaining amount above salvage, with a zero
floor. The preceding book value is obtained algebraically, without scanning or
allocating a schedule. Small declining rates use log1p-based growth; a ratio that
underflows to zero has a separately ordered nominal-charge path. Non-finite book
values fail explicitly. A negative growth base with a nonintegral exponent fails
Domain rather than publishing NaN.

DDB requires nonnegative salvage, positive factor, and `0 < period <= life`.
The provisional Microsoft runtime boundary profile additionally returns zero for
nonpositive cost; for life below two it returns cost minus salvage; at life two it
returns that amount through period one and zero afterward, independently of factor.
For longer lives, periods through one receive the first-period charge. Salvage
above cost can produce a negative first/short-life result; subsequent ordinary
charges have a zero floor. Factors exceeding life can create nonmonotone algebraic
edge cases outside ordinary schedule qualification. These behaviors are explicit
reference choices, not claims of native Access equivalence or accounting advice.

## References and qualification limits

Microsoft's [SLN](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/sln-function),
[SYD](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/syd-function)
and [DDB](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ddb-function)
references define the arguments and Double results. Their positive-input wording
does not establish all runtime boundaries. DDB's displayed equation omits prior
book-value depreciation and cannot alone explain successive declining charges.
Microsoft's [financial runtime reference](https://github.com/dotnet/runtime/blob/main/src/libraries/Microsoft.VisualBasic.Core/src/Microsoft/VisualBasic/Financial.vb)
provides the additional signed-input and short-life reference choices above;
its implementation is not an Access execution oracle. The Rust kernels are
original algebraic implementations; no external source or runtime was adopted.

The ordinary DDB model checks cover positive cost, salvage below cost, integral
life above two and declining factors no larger than life. Separate controls label
signed, short-life, salvage-above-cost and fractional boundaries. Full native
coercion/subtypes, admissibility, short-life/custom-factor behavior, fractional
periods, precision and error order remain unqualified. Extreme intermediate range
and Double cancellation remain limitations. Windows comparisons remain deferred.

## Verification

Six focused expression controls cover independent golden schedules, numeric
carriers, errors, optional syntax and range boundaries. Two properties each use
256 fixed-seed cases: a separate period-by-period book ledger checks ordinary
DDB; an integer digit-weight model checks SYD and straight-line conservation.
A bound query control checks projection dependencies, factor defaults, aggregate/
domain/correlated composition and late failures under TOP.

The original [SavedDepreciationFixture](testing/SavedDepreciationFixture.java)
adds ten inert definitions to the existing fourteen-query forest/data recipe.
The [comparison runner](testing/compare_depreciation_functions.py) passes 278
controls using exact
Fraction schedules, raw native row/metadata getters and independently rendered
SQL, then compares TableCraft scalar, direct, saved and named execution. Stored
cases include explicit parameters, grouping, derived reads, UNION, crosstabs and
buffered invalid-period/NULL failures across Jet 4/ACE 2007/V2016/V2019 files.
Jackcess writes/reads inert metadata and renders SQL; it never evaluates formulas.

Numeric parameters using the shared conversion kernel now also accept
[legacy DATE serials](DATE_NUMERIC.md), with existing numeric domain/NULL rules.
Extended numerical conversion and complete Variant/native behavior remain open.
