# Left, Right and Mid count coercion

Left/Right lengths and Mid's required start/optional length now reuse the checked
Long conversion. Numeric/Boolean, Currency/Decimal, Single/Double and raw legacy
DATE carriers round half to even before range and function-domain checks. Exact
fixed coefficients do not pass through Double. Existing Int64 arguments narrow
with range checks; this materialized profile does not establish native declared
LongLong/Variant coercion. Extended dates, Binary, GUID and expression-only states
remain unsupported.

The primary argument references are Microsoft's
[Left](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/left-function),
[Right](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/right-function)
and [Mid](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mid-function)
Long/Variant-Long definitions. [Conversion rules](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
supply the rounding/range basis. Native Access SQL subtype/error precedence still
needs comparison; these references do not qualify complete Variant behavior.

SQL evaluators also accept numeric Text positions under the execution-local
NumberFormat. Missing policy fails with NumberFormatRequired. Both Mid count
operands share cumulative normalization/output budgets. The pure text APIs,
including apply_with_context, still reject Text count operands: their scan callback
does not supply an allocation-budgeted numeric buffer.

```sh
tablecraft query eval "SELECT Left('abcd',1.5),Right('abcd',CDec(2.5))"
tablecraft query run "SELECT Mid('abcd','1,5','2,5')" --number-format de-de
```

The first query returns `ab` and `cd`; the second returns `bc`. Counts are UTF-16
units. Half-even 2.5 becomes 2; -0.5 becomes zero. Left/Right zero yields empty
Text, negative lengths fail, and oversize lengths clamp to the source. Mid starts
must round to at least one; omitted length retains the suffix. The required start
is checked before converting the optional length, retaining existing domain-error
precedence. Non-finite and overflowing numeric counts fail explicitly.

All written expressions remain eager: Left(NULL,1/0) fails. The existing NULL
source shortcut remains after operand evaluation; it yields NULL without trying
to convert materialized counts. NULL counts for a non-NULL source retain the
existing TypeMismatch profile. These exact precedence/NULL policies remain
provisional pending native comparison.

The shared slice kernel borrows the original source and allocates no source copy.
The evaluator converts count slots into stack values, then charges the final
slice output. Source/input size checks, fallible normalization, cancellation and
shared budgets remain in force. A test permits only three normalization bytes
plus two result bytes while borrowing a six-byte source, and cancels every
observed conversion/scan/output checkpoint. A cut through a surrogate pair still
fails with UnrepresentableText rather than emitting replacement characters.
Source Text/NULL carriers, raw byte-string functions, Space/String, substring
search and replacement retain their separately documented profiles. No blanket
Text coercion or complete string-function compatibility is claimed.

## Verification

The batch passes 803 expression/query tests including doctests, 304 separately
run minimal-profile expression tests and three selected CLI tests. Focused
controls cover count carriers, eager failures, prepared parameter/policy reuse,
derived/correlated/closed Eval calls, borrowed-source allocation accounting and
Unicode boundaries. All-feature/minimal Clippy, warnings-denied rustdoc and
format/diff checks pass.

The original [comparison recipe](testing/compare_slice_counts.py) passes 1,122
independent controls using Decimal nearest-even rounding and strict Python
UTF-16 slicing. Read-only Jackcess getters supply ID references for unchanged
Jet 4/ACE 2007/ACE 16 2016/2019-family files; TableCraft executes the actual
slicing queries. Source/JAR hashes are checked. No third-party data/code is added,
and no native Microsoft Access SQL comparison was run.
