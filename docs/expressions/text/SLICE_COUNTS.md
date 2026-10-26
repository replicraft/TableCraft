# Left, Right and Mid scalar sources and count coercion

Left/Right lengths and Mid's required start/optional length now reuse the checked
Long conversion. Numeric/Boolean, Currency/Decimal, Single/Double and raw legacy
DATE carriers round half to even before range and function-domain checks. Exact
fixed coefficients do not pass through Double. Existing Int64 arguments narrow
with range checks; this materialized profile does not establish native declared
LongLong/Variant coercion. Extended dates, Binary and GUID count operands remain
unsupported; expression-state admission is described below.

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
Nested slices consumed through [lossless UTF-16 observations](BYTE_STRING_FUNCTIONS.md#nested-utf-16-unit-slices)
now retain those units without scalar publication; this uses a separately charged
UTF-16 buffer for Text sources and does not change the borrowed scalar kernel.
Direct outer slices also accept raw-producing sources, publishing only their
selected result; selecting an empty or representable range need not publish an
invalid intermediate first. Materialized scalar sources retain the borrowed path.
The pure kernels retain Text/NULL sources. SQL scalar sources now use the explicit
conversion below. Raw byte-string functions, Space/String, substring search and
replacement retain their separately documented profiles. No blanket
Text coercion or complete string-function compatibility is claimed.

## Verification

The batch passes 803 expression/query tests including doctests, 304 separately
run minimal-profile expression tests and three selected CLI tests. Focused
controls cover count carriers, eager failures, prepared parameter/policy reuse,
derived/correlated/closed Eval calls, borrowed-source allocation accounting and
Unicode boundaries. All-feature/minimal Clippy, warnings-denied rustdoc and
format/diff checks pass.

The original [comparison recipe](../../testing/compare_slice_counts.py) passes 1,122
independent controls using Decimal nearest-even rounding and strict Python
UTF-16 slicing. Read-only Jackcess getters supply ID references for unchanged
Jet 4/ACE 2007/ACE 16 2016/2019-family files; TableCraft executes the actual
slicing queries. Source/JAR hashes are checked. No third-party data/code is added,
and no native Microsoft Access SQL comparison was run.


## Scalar string sources

SQL Left/Right/Mid now accept ordinary scalar sources through the shared
[CStr service](CSTR.md): numeric values require NumberFormat, date values require
DateTextFormat and Boolean values use the existing English spelling profile.
Text sources remain borrowed; Binary/GUID and Error/Empty/Missing expression
states are not implicitly converted. The parser-free kernels remain Text/NULL
services rather than acquiring formatting or locale dependencies.

After all written expressions evaluate eagerly, a NULL source still returns NULL
without count conversion. Other scalar sources format before position conversion;
formatting is required even for a zero-length result. Counts then retain their
existing checked Long conversion and Mid start-before-length validation. The
borrowed slice is copied only after successful validation, using the same shared
allocation/work budget and selected per-value Text ceiling as source formatting.
Cancellation remains checked during conversion, scanning and result allocation.
No prepared spelling is cached; the next execution uses its actual format policy.

Microsoft's Access query examples and string-argument definitions for
[Left](https://support.microsoft.com/en-us/access/functions/left-function),
[Right](https://support.microsoft.com/en-us/access/functions/right-function) and
[Mid](https://support.microsoft.com/en-us/access/functions/mid-function), together
with [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554),
provide the basis. This implementation reuses TableCraft's explicit CStr profile;
complete Variant/native error precedence, regional formatting and lossless
surrogate results remain unqualified.

Three expression and two query tests cover numeric/fixed/date/Boolean sources,
format rebinding, nested positions, NULL/eager failures, surrogate boundaries,
combined allocation budgets and every observed cancellation checkpoint. The
original [scalar source runner](../../testing/compare_slice_scalars.py) passes 336
independent spelling/UTF-16 controls, including sequential and explicit ascending/
descending index reads over the unchanged four-family 20,000-row inputs. Jackcess
supplies raw IDs only; TableCraft alone executes SQL.

The existing count runner also passes 1,086 scalar-mode nearest-even/UTF-16
regression controls after this change; its optional native branch was not rerun.


## Ordered expression-state operands (23 October 2026)

Left, Right and Mid now retain already evaluated expression states until their
own source/position/length admission. Both SQL paths evaluate every written
argument first; a later division-by-zero still propagates when an earlier operand
is Error data, and a volatile argument runs once. Value-only callers share the
same bounded slice kernel without cloning source Text or converting an entire
argument list into owned scalar values.

Host Empty becomes empty Text in the string-source position and Long zero in
count positions. Left/Right zero yields empty Text; Mid start zero remains Domain,
and an Empty optional length yields empty Text. Error data fails TypeMismatch at
the point it is required. Host Missing is NonScalarExpression, distinct from an
omitted optional SQL length. Explicit CStr(CVErr(...)) or CLng(CVErr(...)) remains
a separate requested conversion. Query Nz(NULL) retains its existing Text result;
there is no SQL Empty constructor or new stored carrier.

The existing early raw-Text ceilings and progressive conversion order remain:
source conversion precedes position conversion; Mid validates its start before
admitting a supplied length. A NULL source still returns NULL before control
admission, after every written operand has been evaluated. Consequently
`Left(NULL,CVErr(1))` returns NULL under this explicit profile, while
`Left(NULL,1/0)` still fails. Native Access admission/error precedence remains
unqualified; this extends the existing NULL-source kernel rule rather than
claiming native error-order evidence. UTF-16 boundary checks, scalar formatting
policies and shared work/output/cancellation limits are unchanged.

The reference is [MS-VBAL revision 3.0](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74),
Left/Right/Mid string-expression and Long count contracts with the Error/Empty
Let-coercion rules (5.5.1.2.9/11). Broader Variant/static-width, surrogate publication
and native query qualification remain unfinished. Independent rounded-count and
UTF-16 models, state refusals and optional registered native integrations use
`compare_slice_counts.py --native-directory PATH`; original hashes are checked
before and after reads. These controls do not execute native Access SQL.
