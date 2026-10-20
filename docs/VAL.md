# Val numeric-prefix parsing

Unqualified `Val(text)` now works in scalar SELECTs and bound retrieval. It reads
an explicit numeric prefix; it does not invoke VBA or enable general implicit
Text-to-number coercion. The result currently always uses the finite Double
carrier. NULL is InvalidNullConversion and non-Text input is TypeMismatch.
Wrong arities, qualifiers and missing arguments fail at the existing call boundary.
Written arguments remain eager; complete query/CLI output is buffered on failure.

[Microsoft's Val reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/val-function)
documents prefix termination, removal of spaces/tabs/linefeeds, octal/hex prefixes,
period-only decimal separation and type-suffix checking. Accordingly, spaces,
tabs and linefeeds are skipped inside the prefix; commas and currency symbols
terminate it. A nonnumeric start returns zero. `Val(' 2 45 7')` returns 2457,
`Val('24 and 57')` returns 24 and `Val('&HFFFF')` returns -1.

## Explicit current profile

Decimal prefixes allow a leading sign, one period and an optional E/e or D/d
exponent. An incomplete exponent terminates the mantissa. Carriage returns and
other Unicode whitespace terminate parsing. Underflow may become zero; nonfinite
results fail with Overflow. These edge policies require native qualification.

| Suffix | Current validation before conversion to Double |
| --- | --- |
| `%` | Exact integral Int16 range; fractions fail with TypeMismatch. |
| `&` | Exact integral Int32 range. |
| `^` | Exact integral Int64 range, explicitly available on every TableCraft host. |
| `!` | Checked finite Single narrowing, then represented as Double. |
| `#` | Finite Double. |
| `@` | Four-place half-even Currency rounding and checked signed coefficient range, then the existing Currency-to-Double conversion. |
| `$` | TypeMismatch. |

Integer/Currency suffixes inspect normalized decimal digits before floating-point
rounding. This is lexical range/scale validation, not a separate arithmetic engine;
Currency retains the shared core carrier and conversion policy. A suffix ends the
prefix; text after it is ignored. `50%` succeeds and `50.5%` fails.

Hexadecimal `&H` and octal `&O` prefixes accumulate checked unsigned digits. Without
an integer suffix, up to 16 bits are interpreted as signed Int16 and larger values
as signed Int32. `%`, `&` and `^` explicitly select signed 16/32/64-bit bit patterns.
An optional unary sign applies after interpretation. Overflow and unsupported
coercions fail rather than substituting a value. Radix width/sign/error precedence,
Currency rounding, suffix/exponent corner cases and native result subtypes remain
unqualified. Microsoft restricts LongLong to 64-bit hosts; this implementation's
explicit portable policy is not a native-host emulation claim.

## Resources and verification

The direct kernel returns a borrowed NumericPrefix recipe. Its host supplies an
initially empty NumericBuffer with charged, fallible append and cancellation/work
checks. Decimal normalization charges UTF-8 payload bytes before allocation;
radix accumulation needs no normalization allocation. Scanning and a subsequent
conversion pass charge work. Existing input/output byte, depth and total work
ceilings apply; partial scratch is discarded on error. The buffer starts empty
and must expose exactly the appended contents. No dependency or fixture is added.

Focused tests cover documented examples, exact suffix boundaries, generated Int32
and radix models, rounding, budget exhaustion, cancellation, grouping, parameter
rebinding, aggregates and CLI failure buffering. The [independent recipe](testing/compare_val.py) adds Python integer/Decimal
models and sequential/indexed projections over Jackcess-decoded native rows;
the source database hash is checked after reads. Native Access execution remains
unrun. Broader Variant coercion, exact native result subtypes and native error
precedence are still open; Val is **Partial** in the [tracker](ACCESS_BUILTINS.md).

```sh
tablecraft query eval "SELECT Val(' 2 45 7'), Val('&HFFFF'), Val('24 and 57')"
tablecraft query run "SELECT Val(NBR_VALUE) FROM ROULETTE_TB" --database example.accdb
```
