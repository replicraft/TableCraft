# Bounded Space, String and StrReverse

The shared text kernel supplies these functions to scalar SELECT evaluation and
bound retrieval, including grouped/correlated expressions, reusable parameters
and qualified saved SELECTs. Each returns Text or its documented NULL/error result;
no VBA, locale service or application runtime is invoked.

## Current arguments and behavior

| Function | Current implementation |
| --- | --- |
| Space(number) | Checked half-even numeric-to-Int32 count; nonnegative counts repeat U+0020, zero returns empty Text and NULL returns NULL. |
| String(number, character) | The same count conversion; repeats the first BMP UTF-16 unit of Text, or a nonnegative numeric character code after checked Int32 conversion and modulo 256. Either NULL argument returns NULL. |
| StrReverse(expression) | Reverses BMP UTF-16 units of Text. Empty Text stays empty; NULL is InvalidNullConversion; non-Text operands fail. |

Negative counts fail with Domain and conversion overflow/non-finite input remains
an error. Boolean operands follow the explicit numeric conversion kernel: TRUE
converts to -1, so it is not a valid nonnegative count/code. Text counts, implicit
date conversion and full Variant coercion are absent. All written operands evaluate
eagerly before selection: `String(NULL,1/0)` still fails. Function-specific NULL
propagation after evaluation and conversion/error precedence remain explicit
current policies requiring native Access qualification.

Text character selection uses the first UTF-16 unit, not the whole string. Empty
Text is a Domain error. A supplementary first character would select an unpaired
surrogate and fails with UnrepresentableText, even for zero repeats under the
current validation order. A BMP prefix followed by supplementary characters works
because only the first unit is selected. Numeric codes reduced to 0–127 use the
explicit ASCII profile, including NUL/control characters; reduced bytes 128–255
return CharacterPolicyRequired. No implicit Windows code page or Latin-1 mapping
is selected. Host code-page controls and extended numeric byte decoding are not
yet implemented.

Reversal preserves BMP characters, NUL, spaces and controls without normalization.
Combining marks reverse as independent code units; no grapheme policy is inferred.
Supplementary input currently fails rather than choosing between native surrogate
reversal and Rust scalar reversal. Lossless UTF-16 storage and native supplementary
character qualification remain work to do.

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
coercion, code-page selection/high-byte character codes, lossless surrogate results,
native Unicode/default/error behavior and Windows Access execution remain open.
