# UTF-16 byte observations

LenB and AscB now operate on materialized Unicode Text in scalar and bound
read-only queries. Microsoft explicitly includes both in its
[Access database-engine query sandbox list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode).
They observe the Windows-compatible UTF-16LE representation, independently of
the host machine's encoding or selected ANSI character-code page.

| Function | Current result |
| --- | --- |
| LenB(Text) | Long containing the number of UTF-16 bytes, with checked overflow |
| LenB(NULL) | NULL |
| AscB(nonempty Text) | Integer containing the first UTF-16LE byte, in 0..255 |
| AscB(empty Text) | Domain error |
| AscB(NULL) | InvalidNullConversion |

The [Len/LenB specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/a4d9985a-e2cb-489c-a20a-5da09d77a59e)
defines byte length and NULL propagation. Microsoft's
[Access field-size remarks](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-fieldsize-property-dao)
distinguish Unicode LenB from encoded/compressed field size. The
[Asc reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/asc-function)
defines AscB as observing the first byte, rather than performing Asc's character
code conversion. Here that means the low byte of the leading UTF-16 unit.

```sh
tablecraft query eval "SELECT Len('é中🦀'),LenB('é中🦀'),AscB('中')"
```

Results are 4 UTF-16 units, 8 bytes and byte code 45. The UTF-8 payload occupies
9 bytes; neither function reports that length. The supplementary crab character
occupies two UTF-16 units, so LenB counts four bytes for it. A leading `Ā` has
first byte zero; no zero-terminated-string interpretation is applied. Combining
marks are retained without normalization.

## Scope and metering

Both calls require one ordinary expression. Missing slots, extra arguments,
wildcards and qualified names fail before operand evaluation. Existing input
Text ceilings, eager errors and buffered result/stdout behavior remain unchanged.
LenB charges work while counting characters, then checks the byte-width multiply.
AscB reads one leading unit without heap allocation. Both return existing scalar
carriers, with no additional output-string allocation or new dependency.

The private text `byte_string` slice owns these observations. LenB/AscB do not
reinterpret Binary arrays as strings, parse numeric/date carriers, or implement
VBA variable/UDT storage-size overloads. Numeric and other non-Text carriers fail
TypeMismatch. Full Variant coercion and native platform/error behavior remain
unqualified, so both functions remain **Partial** in the
[tracker](ACCESS_BUILTINS.md).

## Remaining byte-string work

LeftB, RightB, MidB, ChrB and InStrB remain unimplemented. Byte boundaries can
produce odd-length or unpaired UTF-16 data that valid-Unicode Text cannot retain.
Those functions need a lossless expression representation and explicit conversion,
comparison and publication rules. An ANSI conversion through StrConv is another
operation; it must not silently change LenB/AscB's Unicode input representation.
No surrogate replacement, implicit code-page selection or Binary/Text alias is
introduced by this implementation.

## Evidence

Kernel tests include independent Unicode/NUL/combining/supplementary goldens,
code-page independence, NULL/carrier rules, scan cancellation and a fixed-seed
256-case code-point model. Query/CLI tests cover numeric subtypes, fields,
filters/groups/correlations, aggregates, parameter reuse, invalid calls and eager
failure with empty stdout.

The original [independent runner](testing/compare_byte_strings.py) models results
with Python UTF-16LE encoding. It also uses reviewed Jackcess getters on the
[original Hyperlink field fixtures](FIXTURES.md#original-hyperlink-field-values)
before TableCraft queries the decoded values, with unchanged hashes. This does
not establish native Access evaluation or compressed/native storage sizes.
