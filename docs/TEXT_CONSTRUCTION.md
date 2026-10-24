# Bounded Space, String and StrReverse

The shared text kernel supplies these functions to scalar SELECT evaluation and
bound retrieval, including grouped/correlated expressions, reusable parameters
and qualified saved SELECTs. Each returns a String or its documented NULL/error result;
no VBA, locale service or application runtime is invoked.

## Retained String construction

The owning evaluators now retain both String operands until the function's NULL
gate and count/character admission. Count conversion still uses checked Long
rounding and explicit numeric Text policies; a retained count publishes only when
conversion is actually required. NULL returns NULL without decoding an unused
raw operand. Operational failures in written arguments remain eager.

String selects the first complete UTF-16 unit from ordinary Text or an active
raw String range. The source byte envelope must fit signed Long and have an even
length. Empty sources are Domain errors and odd ranges are refused, including
for zero repetitions. Complete source tails are not decoded or copied; an
unselected isolated unit does not invalidate a BMP first unit. Unaligned active
ranges retain their little-endian unit interpretation.

A valid scalar unit keeps the existing streamed UTF-8 construction path. A
surrogate unit uses bounded raw-byte output, with per-append allocation/work/
cancellation and the existing signed-Long String ceiling. There is no eager
count-sized allocation. `apply_retained_string` exposes this result explicitly;
materialized evaluator output remains strict. The legacy parser-free Text recipe
retains its BMP publication profile. Numeric character operands still use their
existing Long/modulo-256 and explicit code-page conversion, including at count
zero. Host Empty characters retain numeric zero rather than empty Text.

For example, `LenB(String(3,ChrW(55296)))` returns 6,
`AscW(String(2,'😀'))` returns -10179, and
`String(1,ChrW(55357))&ChrW(56832)` repairs a pair and publishes valid final Text.
Derived/grouped/correlated cells, prepared values, domain retrieval and String
inspection can consume retained results. A directly selected unpaired result
still fails before buffered output reaches the user. String keeps its existing
fresh result text policy; it does not activate the character source's comparer.
Stored/source/parameter carriers and write gates are unchanged.

Microsoft's specific [String query example](https://support.microsoft.com/en-us/access/functions/string-function)
establishes direct query availability and its first-character/modulo/NULL contract.
The unit, coercion and error-edge profile above remains independent engineering
behavior pending native Access qualification.

The [original runner](testing/compare_raw_repetition.py) passes 2,432 independent
controls using Python unit-list repetition, exact binary observations, seeded
units, complete tails, rounded counts, zero/NULL outcomes and refusals. Four
unchanged native inputs supply counts and character fields through inert getters;
TableCraft executes sequential/selected-index queries under stored and explicit
General choices. Existing state and rounded repetition/search recipes pass 220
and 1,200 controls. No new dependency, benchmark or Windows Access comparison
is supplied by these checks.

## Current arguments and behavior

| Function | Current implementation |
| --- | --- |
| Space(number) | Checked half-even numeric-to-Int32 count; nonnegative counts repeat U+0020, zero returns empty Text and NULL returns NULL. |
| String(number, character) | The same count conversion; repeats the first complete UTF-16 unit of Text or a retained String, or a nonnegative numeric character code after checked Int32 conversion and modulo 256. Either NULL argument returns NULL. |
| StrReverse(expression) | Reverses BMP UTF-16 units of Text. Empty Text stays empty; NULL is InvalidNullConversion; non-Text operands fail. |

Negative counts fail with Domain and conversion overflow/non-finite input remains
an error. Boolean operands follow the explicit numeric conversion kernel: TRUE
converts to -1, so it is not a valid nonnegative count/code. Legacy DATE counts
and numeric character operands now use their raw serial's checked Long conversion.
SQL numeric Text counts require the explicit number format; full Variant and
source/character string coercion remain incomplete. All written operands evaluate
eagerly before selection: `String(NULL,1/0)` still fails. Function-specific NULL
propagation after evaluation and conversion/error precedence remain explicit
current policies requiring native Access qualification.

Text character selection uses the first UTF-16 unit, not the whole string. Empty
Text is a Domain error. A supplementary first character selects its high
surrogate, which the owning evaluator now retains through raw-aware consumers.
Zero repetitions produce an empty String after character validation. Direct
publication of a nonempty unpaired result remains UnrepresentableText. A BMP
prefix followed by supplementary characters works because only the first unit
is selected. Numeric codes reduced to 0–127 use the
explicit ASCII profile, including NUL/control characters; reduced bytes 128–255
return CharacterPolicyRequired unless an [explicit supported code page](CHARACTER_CODE_PAGES.md)
is selected. `--character-code-page ID` on query eval/run and matching library
controls now decode supported high bytes without an implicit Windows page or
Latin-1 mapping. Unmapped selected-page bytes fail without replacement.

Reversal preserves BMP characters, NUL, spaces and controls without normalization.
Combining marks reverse as independent code units; no grapheme policy is inferred.
Supplementary input currently fails rather than choosing between native surrogate
reversal and Rust scalar reversal. Extending reversal and qualifying native
supplementary behavior remain work to do.

```sh
tablecraft query eval "SELECT Space(3), String(5,42), String(3,'éx'), StrReverse('abc')"
tablecraft query run "SELECT Len(String(4,'x')) AS Width"
```

The first returns three spaces, five asterisks, `ééé` and `cba`. The second returns
Int32 4. CLI output retains its existing quoted Text display and CSV/TSV encoding.

## Construction and resources

The parser-free kernel returns a borrowed `Construction` recipe. Its writer uses
the shared `functions::text::output::TextOutput` trait, which also serves Replace.
The existing `replacement::TextOutput` import remains available as a re-export.
The recipe owns no intermediate String and uses a stack UTF-8 character buffer.
Each emitted/scanned character charges work; the host sink checks cancellation,
per-value UTF-8 size and cumulative bytes before fallible output growth.

A count as large as Int32::MAX does not trigger an eager unbounded repeat allocation.
The evaluator stops at its applicable work/byte/value ceiling and discards partial
output on error. Direct library sinks must enforce their own limits and discard
partial output. Payload budgets are cumulative work allowances, not exact allocator
capacity/resident-memory accounting. Normal input/depth limits and buffered query
publication remain in force. These are correctness/resource controls, not measured
throughput claims.

## References and evidence

Microsoft's [Space](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/space-function),
[String](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/string-function)
and [StrReverse](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strreverse-function)
references guide repetition, first-character selection, numeric modulo behavior,
NULL outcomes and reversal. They do not qualify this Unicode/code-page profile
or a Windows execution result.

Five kernel tests, including generated BMP repeat/reversed-buffer models, cover
counts, NULLs, conversion, control characters, modulo codes, domain/code-page/
surrogate refusals and bounded/cancelled output. An evaluator test checks per-value,
cumulative-byte and step limits. Two query tests cover binding, grouping,
correlation, parameter reuse, eager failures and wrong arities. A CLI test decodes
CSV/TSV transport to check results and buffered errors.
The [independent recipe](testing/compare_text_construction.py) uses Python repetition
and reversed UTF-16 buffers, explicit refusal controls and independently decoded
sequential/indexed native rows. Source database bytes remain unchanged; check totals
are recorded in [implementation status](IMPLEMENTATION.md).

All three remain Partial in the [built-in tracker](ACCESS_BUILTINS.md). Complete
coercion, broader/native code-page mapping, lossless reversal results,
native Unicode/default/error behavior and Windows Access execution remain open.

## Contextual repetition counts

`Space('2,5')` under de-de now emits two spaces; `String('2,5','65')` emits `66`.
Only the count slot uses numeric Text conversion. A Text character operand remains
a literal first-unit source, even when its spelling looks numeric. Numeric DATE
character operands retain the existing Long/modulo-256 path. Pure kernels still
reject Text counts rather than infer a regional policy.

The evaluator normalizes counts into stack numeric values, then shares the
existing streamed construction recipe. It copies no character/source Text to
replace a count argument. Normalization, output, cancellation and input ceilings
share the operation budget. A five-byte budget permits three normalized count
bytes plus two result bytes while borrowing a six-byte character source. Existing
NULL propagation occurs before materialized count conversion, while all written
argument expressions remain eager. Domain/character-page and complete-unit checks still
apply for zero counts; StrReverse behavior is unchanged.

The corresponding [search-start extension](STRING_SEARCH.md#contextual-search-starts)
retains its different defaults and NULL precedence. The combined batch passes
812 expression/query tests, 306 separately run minimal-profile tests and four
selected CLI tests. The original [comparison recipe](testing/compare_repeat_search.py)
passes 1,200 independent repetition/UTF-16/rounding controls, including read-only
ID getter integration over unchanged Jet 4/ACE 2007/ACE 16-family sources. Only
TableCraft executes SQL. Native Access coercion, subtype, NULL and code-page
qualification remains open.


## Scalar reversal sources

SQL StrReverse now uses explicit CStr policies for ordinary non-Text scalar input.
Numeric/date formats remain caller-issued, Boolean spelling uses the existing
profile, and Text remains borrowed. The existing BMP code-unit reversal recipe
then writes through the same bounded sink. Source formatting and final output
share byte/work/Text/cancellation allowances. NULL remains InvalidNullConversion,
empty Text remains empty and supplementary input remains UnrepresentableText.
Binary/GUID remain refused. StrReverse now uses the shared
[unary string state admission](CSTR.md#unary-string-expression-states), including
host Empty as empty Text and explicit Error/Missing refusal.

Microsoft's [Access StrReverse reference](https://support.microsoft.com/en-us/access/functions/strreverse-function)
and [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
provide the string-argument/conversion basis. This reuses the existing CStr and
reversal profiles; complete Variant/regional/native error precedence and lossless
supplementary reversal remain compatibility gaps.

Three expression and two query tests cover scalar character-code/reversal inputs,
format rebinding, nested calls and exact Int16 results, NULL/empty/provider errors,
surrogate behavior, shared byte quotas and every observed cancellation checkpoint.
The original [runner](testing/compare_character_reverse_scalars.py) compares
independent scalar spellings, ASCII codes, type tags and reversal results through
both scalar CLI modes and four-family sequential/indexed native reads. Originals
remain unchanged; native Access SQL qualification remains pending.

## Ordered repetition states

Both scalar SELECT evaluation and bound queries now retain Error/Empty/Missing
operands through Space/String admission. Written arguments are evaluated once,
in order, before this admission; operational failures still stop immediately.
Arity, existing input Text limits and cancellation apply before the owning gate.

String's existing NULL rule runs before count/character conversion, including
`String(NULL,CVErr(11))` and `String(CVErr(11),NULL)`. Without NULL, the count uses
the existing checked Long conversion. Host Empty selects numeric zero; Error
data gives TypeMismatch and Missing gives NonScalarExpression. A negative count
returns Domain before admitting the character. Text character sources retain
literal first-unit selection; numeric/DATE sources retain Long/modulo behavior.
Host Empty characters select numeric zero (NUL), while Error/Missing characters
remain refused even for a zero count. Empty Text remains a domain error. No SQL
Empty constructor is added; query Nz(NULL) still produces its existing empty Text.

The new expression API borrows Text sources and shares the original construction
recipe and bounded text-result materializer. It does not allocate a second
character/source buffer, replace Error data with NULL or implicitly use its error
code. Explicit `CLng(CVErr(...))` or `CStr(CVErr(...))` remains a separate operation.
Step/byte/Text/cancellation allowances remain cumulative. No return-category,
code-page, surrogate, provider or native compatibility claim changes.

Microsoft's [String argument/NULL contract](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/string-function),
[Space contract](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/space-function)
and [numeric Empty conversion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e2637723-3010-449d-bead-9f17533dc105)
guide this existing query-function profile. These references do not establish
native Access Error/NULL precedence or full Variant character coercion.

Three expression tests cover state/gate order, borrowed character conversion,
shared byte limits, every observed successful cancellation checkpoint and
once-only Rnd consumption after earlier Error data. A query test adds source
NULLs, derived Error values, aggregate consumption and eager failures.
A CLI test verifies both scalar commands and empty stdout on failure. The
[independent runner](testing/compare_repetition_states.py) passes 220 controls
across scalar modes and registered four-family sequential/indexed native reads;
all source hashes remain unchanged. Only TableCraft executes SQL. Native Access
qualification, full Variant promotion and broader query semantics remain open.
