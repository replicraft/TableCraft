# IsNumeric scalar inspection

Unqualified `IsNumeric(value)` now returns Boolean in scalar SELECTs, bound
retrieval and qualified saved SELECT execution. It has one required actual
expression. Wrong arities, missing slots, wildcards and qualifiers fail before
operand execution. Written arguments remain eager; a division error or failure
inside an IIf branch is not turned into FALSE. No VBA/runtime content executes.

| Materialized input | Current result |
| --- | --- |
| Boolean, Byte, Int16, Int32, Int64, Single, Double, Currency, Decimal | TRUE, by numeric/Boolean type rather than truthiness or target conversion. |
| NULL, legacy/extended date, binary, GUID | FALSE. NULL does not propagate. |
| Text | Requires an explicit NumberFormat and recognizes the entire supported numeric spelling. |

Boolean FALSE and numeric zero are still numeric types. Typed IEEE carriers are
classified by type even if they contain nonfinite bits; this does not make their
arithmetic valid or permit invalid numeric literals to materialize. Text recognition
uses the [shared whole-string profiles](NUMBER_FORMAT.md) and finite Double range.
Valid underflow to zero is recognized as numeric. The result does not guarantee
that a narrower target such as CInt or Currency would accept the value.

```sh
tablecraft query eval "SELECT IsNumeric(0), IsNumeric(False), IsNumeric(NULL)"
tablecraft query eval "SELECT IsNumeric('53'), IsNumeric('45 Help')" --number-format invariant
tablecraft query run "SELECT ORDR_NBR FROM ROULETTE_TB WHERE IsNumeric(NBR_VALUE)" \
  --database docs/examples/Access_Example_VBA.accdb --number-format invariant
```

The first returns TRUE, TRUE, FALSE; the second returns TRUE, FALSE. Text without
a policy raises NumberFormatRequired, including empty Text. Date strings, radix prefixes/type suffixes, unsupported grouping/currency
patterns and full Variant/locale coercion remain outside the current grammar. Native range,
underflow, syntax, type and error precedence remain unqualified. IsNumeric is
**Partial** in the [built-in tracker](ACCESS_BUILTINS.md).

## Failure and query boundaries

Only malformed numeric text and numeric text overflow produce FALSE. Allocation,
byte/work/input limits, cancellation, missing policy and other failures remain
errors. A checked buffer records host-error origin before interpreting parser
failures, so a callback failure cannot be swallowed as a lexical TypeMismatch or
Overflow. Partial normalization storage is private and discarded. Complete query
and CLI output remains buffered on failure.

The parser-free inspection kernel lives in tablecraft-expressions. The evaluator
supplies shared limits, cancellation and per-execution policy. A typed backend-neutral
Inspect node participates in binding, field pruning/remapping, grouping identity,
aggregate detection, correlations and qualified multivalue parent predicates.
The original filter remains at execution; this does not add native/SQLite predicate
pushdown or change index selection. Inspection does not authorize calls in the
bounded UPDATE profile. Prepared plans do not capture a numeric format and the
optional worker reuses only request-local settings.

IsNumeric is not a short-circuit guard for an eager conversion: a predicate such
as `IsNumeric(Label) AND CLng(Label)>0` can still fail when Label is invalid. This
function does not change existing AND or IIf evaluation rules.

## References and evidence

Microsoft's [IsNumeric function guide](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isnumeric-function)
explicitly demonstrates successful recognition of numeric strings and rejection
of trailing words, and says date expressions are false. The
[MS-VBAL IsNumeric page](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d920bc45-c366-4151-97dc-58c6a5b82780)
lists numeric/Boolean types as true and all others as false, without the guide's
numeric-string case. TableCraft follows the function guide for Text and the formal
numeric/Boolean type cases for materialized carriers. This reference discrepancy
and actual ACE expression behavior still require native comparison.

Four kernel tests cover all current carriers, Boolean versus numeric Text, current
profiles, invalid/range cases, generated integer text and injected host failures.
An evaluator test checks allocation, work, input-size and cancellation failures.
Two query tests cover selective fields, grouping, aggregates, correlated arguments,
parameter/policy reuse, invalid grouping and eager errors. A multivalue test checks
hidden member dependencies and parent-row behavior. CLI transport/failure buffering
and optional worker request isolation have focused regressions.
The [independent recipe](testing/compare_inspection.py) uses generated Python
Decimal spellings, explicit type/negative controls and optional sequential/indexed
native-row projections with an unchanged source hash. These are independent-model/
reader checks; native Windows Access execution remains unrun. No dependency or
fixture is added. Array/object and expression-only Empty/Missing/Error states are
not materialized by this scalar model; evaluation failures are not Variant Error.


## Expression-state recognition (23 October 2026)

IsNumeric now retains expression states through eager evaluation in both scalar
and relational SQL. Error data and host Empty return FALSE without numeric-code
conversion, number-format lookup or payload allocation. Host Missing remains
NonScalarExpression. An explicit `CLng(CVErr(1))` supplies a numeric value and
therefore returns TRUE. Query `Nz(NULL)` keeps its separate empty Text identity
and existing Text-policy requirements; no SQL Empty constructor is added.

This follows the Error/Empty distinction in
[MS-VBAL IsNumeric](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d920bc45-c366-4151-97dc-58c6a5b82780).
The existing numeric-Text recognition profile remains as documented above;
recognition does not silently broaden coercion. Work and cancellation checks still
run for state-only results. Division-by-zero, provider/policy failures and resource
errors propagate; the implementation does not catch arbitrary evaluation errors
and turn them into FALSE. Its formal Scalar Boolean category is unchanged.

Derived, correlated, grouped, aggregate/domain and scalar-subquery consumers use
the same entry point. Independent source-free/state and source-bound native-field
controls are available through `compare_inspection.py --native-directory PATH`.
The optional directory uses registered original index fixtures and verifies hashes
before and after execution. Native Access result/precedence qualification remains
pending; ordinary SQL aggregate/UNION declaration categories remain unknown.
