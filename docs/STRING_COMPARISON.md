# StrComp and explicit comparison policies

`StrComp(left, right[, compare])` shares a closed text kernel between scalar
SELECT evaluation and bound retrieval, including grouping, correlations, reusable
parameters and qualified saved SELECTs. It returns Int16 -1/0/1 or NULL. Every
supplied argument evaluates in written order; NULL strings do not hide errors in
another argument. No VBA or external function is invoked.

## Lossless binary String operands

StrComp now retains raw String operands until mode-specific comparison. Binary
mode 0 compares complete unsigned UTF-16 units directly, including unpaired
surrogates, without Unicode decoding, byte alignment casts or payload allocation.
Ordinary Text and raw ranges use the same metered unit-order loop. Active ranges
are read exactly as UTF-16LE units: `MidB('😀',2,2)` compares equal to `ChrW(216)`;
the range is not realigned to its original character boundary.

`StrComp(ChrW(55296),ChrW(57343),0)` returns -1. Odd byte lengths remain
UnrepresentableText when a non-NULL unit comparison is required. Mode validation
still precedes the NULL gate, so `StrComp(ChrB(65),NULL,0)` returns NULL while an
invalid mode still fails. Every operand and the optional mode evaluate eagerly.
Scalar formatting and numeric Text modes retain explicit execution policies.

The owning evaluator exposes `apply_expression_byte_comparison`; the borrowed
`compare_binary_utf16` kernel consumes already admitted String operands and a
caller-owned work callback. An explicitly supplied Binary Option Compare policy
can select this path without a Unicode provider callback. Text/Database modes
retain strict Text publication and their existing providers; this adds no locale
fallback or native Windows qualification. Ordinary predicates, keys and raw
relational storage remain separate.

The original [raw comparison runner](testing/compare_raw_string_comparison.py)
passes 1,102 source-free controls against independent unsigned-unit sequence
ordering, including mixed ordinary Text, NULs, prefixes, trailing spaces, NULL and
refusals. Native Access comparisons and benchmarks were not run.

## Modes and host context

| Compare argument | Current implementation |
| --- | --- |
| 0 | Allocation-free lexicographic comparison of UTF-16 code units; no host required. |
| 1 | Explicit host `Collation::compare_text` callback. No default locale is inferred. |
| 2 | Explicit host `Collation::compare` database-policy callback. |
| Omitted or -1 | Requires the host's declared `Collation::option_compare` policy for non-NULL strings. A supplied database comparer alone does not imply a module setting. |

An omitted trailing SQL argument slot means omission, not stored NULL. Explicit
NULL comparison mode is InvalidNullConversion. Other supplied modes undergo
checked half-even numeric/legacy DATE-to-Int32 conversion; SQL numeric Text uses
the explicit NumberFormat. Only -1, 0, 1 and 2 are accepted.
Unsupported mode codes return Domain; conversion overflow/type errors remain
visible. Evaluator-backed operands accept Text/NULL and supported scalar CStr
conversion under explicit formatting policies, as described below. Empty, Error
and full Variant coercion remain outside this extension. After mode validation,
either NULL string returns NULL without requiring conversion or a comparison provider.

Mode 0 preserves embedded NUL, trailing spaces and canonically distinct Unicode
spellings. It compares UTF-16 units, so supplementary characters can order
differently from Rust Unicode scalar ordering. It does not slice strings or
construct unpaired surrogates. This is an explicit current binary profile,
**not qualification of every native Windows code-page/binary sort behavior**.

Library callers use `functions::text::apply_with_collation`; the existing `apply`
API remains available without a host. Binary scans charge work per compared pair.
Host modes scan/charge both inputs before calling the provider; the provider owns
its callback work/cancellation bounds. After a successful text/database callback,
the kernel invokes the host step check again before returning the comparison.
Cancellation or an exhausted shared step allowance at that boundary therefore
refuses the result. A callback error still propagates before this success check.
Evaluator inputs retain normal text-size,
step-budget and cancellation limits. The comparison result allocates no Text
payload; evaluator-backed scalar operands may allocate bounded formatting intermediates.

The CLI's explicitly selected `--collation ordinal` supplies the database policy
and declares that policy for omitted/-1 modes. It compares Unicode scalar order,
which is intentionally distinct from mode 0's UTF-16 profile. It does not supply a
locale-aware mode 1. The explicit `general-legacy`/`general-2010` CLI choices
likewise supply database/default comparison through the provisional
[full-string weight model](NATIVE_TEXT.md#general-query-policy), under a separate
cumulative budget; they do not supply mode 1. Without an explicit policy,
non-NULL omitted/host modes fail rather than guessing a locale or native database
sort order.

```sh
tablecraft query eval "SELECT StrComp('ABCD','abcd',0) AS Comparison"
tablecraft query run "SELECT StrComp('ABCD','abcd',2) AS Comparison" --collation ordinal
```

Both return -1 in these ASCII examples. CLI results remain buffered; unsupported
modes/providers and late evaluation failures publish no partial table.

## Evidence and remaining compatibility work

Microsoft's [StrComp reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strcomp-function)
defines result codes, NULL behavior and comparison modes. Its
[Option Compare reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/option-compare-statement)
distinguishes module defaults, system text locale and database locale, and notes
Windows code-page effects. TableCraft does not infer a VBA module or database
locale from an opened file. Native code-page binary behavior, full Variant
coercion, locale-aware text comparison and database collation remain incomplete.

Focused value/evaluator/query/CLI tests cover separate provider callbacks, omitted
slots versus NULL, exact result carriers, UTF-16 ordering, grouping, correlations,
parameter reuse, eager errors, budgets and cancellation. A generated property
compares the binary kernel to independently constructed big-endian UTF-16 buffers.
The original [reference recipe](testing/compare_strcomp.py) passes 85 controls
against Python UTF-16/ordinal models and read-only independently decoded native
rows, including sequential/indexed reads. The registered Access example's source
hash remains unchanged. These are independent model/read checks; Microsoft Access
execution on Windows remains unrun. See the [coverage tracker](ACCESS_BUILTINS.md).

[Substring search](STRING_SEARCH.md) reuses mode selection but has its own
`Collation::find` provider contract; supplying compare/compare_text alone does not
implement locale substring matching.


## Database and system locale are separate

Microsoft's Option Compare reference distinguishes Text (system locale) from
Database (database locale). `StrComp(...,1)` must therefore not inherit a database
General descriptor merely because one was selected for SQL predicates. The
[DAO CollatingOrder reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao)
also permits a field to have a different order from its containing database.
TableCraft preserves these separate inputs; it does not use the current machine's
preferred UI language as a database comparison setting. The current binary
function profile uses UTF-16 units rather than treating strings as ASCII.

## Contextual comparison-mode conversion

SQL StrComp, InStr, InStrRev, Replace and InStrB now convert numeric Text mode
arguments under the execution-local number format. Pure kernels also accept raw
legacy DATE modes through the checked Long conversion, but still reject Text
modes rather than allocate unmetered numeric buffers. Fractional values round
half to even; a Text mode of `0,5` under de-de selects binary mode zero.

Text operands retain their spelling: `StrComp('10','2','0,5')` still compares
literal strings and returns -1. Supported scalar operands now use the separate
formatting path below; required comparison providers remain unchanged. InStr alone retains structurally checked positive LCID dispatch
to find_locale; InStrRev/StrComp/Replace retain closed mode sets, and InStrB still
rejects unsupported byte comparison/LCID providers. No new native locale behavior
or default fallback is supplied.

Validation order remains function-specific. StrComp and searches validate modes
before their string NULL checks; search modes also precede starts. Replace
converts start/count first, then its mode, domains and string carriers. Malformed
Text, missing number policy, explicit NULL, overflow and operational failures
remain errors, including for otherwise empty/NULL results. All written operands
remain eager. Normalization and scans share limits without cloning source strings;
a direct control permits one normalized mode byte while comparing two six-byte
inputs, and cancels every observed conversion/comparison checkpoint.

[Character/byte numeric arguments](CHARACTER_FUNCTIONS.md#contextual-character-code-conversion)
share the same selected conversion policy while retaining different domains.
The combined batch passes 828 expression/query tests, 309 separately run minimal
expression tests and three selected CLI tests. The original
[comparison recipe](testing/compare_mode_character.py) passes 840 independent
Decimal/character-code/codec/mode/native-field controls. Native Access comparison
is unrun; full Variant/source coercion and NLS/LCID provider qualification remain
open. No dependency or runtime is added.

## Contextual scalar string operands

Scalar SQL and bound queries now format non-Text StrComp operands through the
existing bounded CStr service. Boolean uses True/False; numeric carriers require
NumberFormat and date carriers require DateTextFormat. Numeric conversion preserves
the existing exact integer/fixed-point spellings and explicit floating formatting
profile; date formatting keeps its existing rounding/calendar rules. Binary/GUID
remain outside this string-formatting path. Expression states follow the ordered
admission below.
The low-level Text/NULL comparison API retains its original contract.

Both operands are already evaluated in written order before this conversion.
The optional mode is validated first, then either NULL returns NULL without
formatting the other operand. Remaining scalar operands convert left to right.
Existing Text is borrowed without another copy; only converted operands allocate,
under the same cumulative byte/work limits and text ceiling as explicit CStr.
Comparison then uses the existing binary or explicitly supplied host mode.
`StrComp(10,2,0)` returns -1 under an explicit number profile: this is a string
comparison, not numeric comparison. The resulting carrier remains Int16.

Prepared executions use current formatting policies; no formatted operand is
cached across executions. Missing formatting/comparison providers, invalid modes,
nonfinite input and eager failures remain explicit, with buffered publication.
No ambient locale is read. This adds no text-mode/LCID provider or native text
precedence rule.

Microsoft's [Access StrComp reference](https://support.microsoft.com/en-au/access/functions/strcomp-function)
and [MS-VBAL StrComp](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9541f02a-e173-47a7-8c0f-4d91813c1c88)
define string operands, result codes and NULL behavior. This bounded materialized
conversion follows the existing CStr contract; complete native Variant conversion
and exact error precedence still require Access comparisons.

Two expression and two query tests cover exact large integers, Boolean, fixed-point
and date spellings, policy changes between executions, numeric-versus-string
ordering, NULL/mode/eager boundaries, byte quotas and every observed cancellation
checkpoint. The original [scalar runner](testing/compare_strcomp_scalars.py) passes
620 independent spelling/UTF-16/native-source controls. It reads raw IDs from the
four unchanged registered 20,000-row inputs and compares sequential and both
explicit ID-index directions. The existing StrComp runner still passes its 85
Text/NULL/provider controls. Neither oracle executes native Access SQL.

## Ordered expression-state admission (23 October 2026)

Both SQL evaluators capture every written StrComp operand before admission. The
optional comparison mode is admitted and validated before the existing NULL gate;
remaining string operands are admitted and formatted left to right. Error data
fails TypeMismatch where a mode/string is required; host Missing stays
NonScalarExpression. A NULL operand can bypass its partner's state admission,
but never suppresses a failure while evaluating a written operand. A missing
number-format policy on a numeric left operand precedes a right Error state.
Explicit CStr(CVErr(...)) retains its separate requested conversion.

Host Empty becomes empty Text in string positions and numeric zero in the mode
position. An Empty mode selects binary comparison; an omitted mode still requires
the host comparison context. No SQL Empty constructor or stored state is added.
Raw Text limits are checked before mode/NULL shortcuts. Value-only calls share
the same ordered helper and unchanged borrowed UTF-16/provider comparison kernel,
with cumulative work, formatting and cancellation controls preserved.

Microsoft's [StrComp query example](https://support.microsoft.com/en-us/access/functions/strcomp-function)
explicitly uses SELECT over ProductSales. That establishes direct query scope;
the broader [MS-VBAL conversion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
inform the explicit host Empty/Error contract. Native mixed NULL/Error precedence,
complete Variant coercion and locale/provider equivalence remain unqualified.
