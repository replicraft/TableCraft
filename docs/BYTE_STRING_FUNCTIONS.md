# UTF-16 byte strings and observations

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

The text `byte_string` feature slice owns these observations and the lossless
byte-string carrier. LenB/AscB do not
reinterpret Binary arrays as strings, parse numeric/date carriers, or implement
VBA variable/UDT storage-size overloads. Numeric and other non-Text carriers fail
TypeMismatch. Full Variant coercion and native platform/error behavior remain
unqualified, so both functions remain **Partial** in the
[tracker](ACCESS_BUILTINS.md).

## Lossless byte producers

ChrB, LeftB, RightB and MidB now execute in scalar and bound read-only queries.
Microsoft's query sandbox list establishes query availability separately from
these function contracts:

- [ChrB](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/900323ac-8160-42cd-ae52-4073f2a95116)
  produces exactly one byte and reports Overflow outside 0..255.
- [Left/LeftB](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/left-function)
  and [Right/RightB](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/right-function)
  select a byte count, returning the whole input if the requested count exceeds it.
- [Mid/MidB](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/4a65ee4e-b6b9-45d3-a3f6-576fed4bb227)
  uses a one-based byte start and optional byte length, returning the remaining
  suffix when length is omitted and empty output beyond the end.

The immutable expression-only `ByteString` owns an already charged buffer plus
an active range. Consuming slices change the range without copying. Odd lengths,
zero bytes, unpaired surrogates and byte pairs assembled across original UTF-16
boundaries survive unchanged. No Clone implementation hides an allocation.
Debug shows only length. This type does not change core stored `Value` carriers.

Unicode Text inputs encode explicitly as UTF-16LE. The evaluator scans and checks
work/cancellation before charging and fallibly reserving the exact byte buffer,
then meters the encoding pass. Raw byte intermediates avoid repeated encoding.
Positional arguments use checked numeric-to-Long conversion. Negative lengths,
nonpositive MidB starts and NULL numeric arguments fail explicitly. Written
operands evaluate eagerly in order; numeric validation precedes propagating a
NULL string. Native coercion and NULL/error precedence remain unqualified.
Only ordinary positional arguments are accepted; missing required slots and
explicit missing lengths are unsupported in the current SQL profile.

Byte producers, CVar, LenB/AscB and type inspection can consume these intermediates
without Unicode publication. TypeName returns String, VarType returns 8 and
IsArray returns False, including for ChrB's single-byte result. Array bounds reject
strings. For example:

```sh
tablecraft query eval "SELECT LenB(ChrB(255)),AscB(RightB('🦀',3)),VarType(ChrB(0))"
tablecraft query run "SELECT MidB('A中🦀',3,6) AS Suffix"
```

The first query returns 1, 216 and 8. The second returns ordinary Text `中🦀`.
Publication as a scalar cell strictly decodes the active UTF-16LE bytes through
the existing bounded TextOutput sink. Odd byte counts and unpaired surrogates
raise UnrepresentableText; partial output is discarded and CLI stdout remains
buffered. An even byte slice starting at an odd UTF-16 boundary can still be valid
Unicode: MidB('🦀',2,2) contains bytes D8 80 and publishes U+80D8. Publication does
not realign a byte slice to the original character boundaries.

## Remaining byte-string work

[StrConv](STRCONV.md) now supports explicit casing and strict single-byte
Unicode/ANSI conversions; width/kana and broader mapping behavior remain open.
Its encoding results use these same lossless byte-string intermediates. InStrB
supports the explicit binary profile below. The current lossless paths do not yet
extend to arbitrary conditional/operator arguments, Len's raw-string overload,
array elements, parameter/result carriers, derived-table cells or raw-string
comparison/sorting. Those boundaries still require ordinary representable Text;
they do not silently replace or truncate invalid data. Dollar-suffixed aliases,
full Variant coercion and native Access behavior remain unqualified or unsupported.
The four producers therefore remain **Partial**, alongside LenB/AscB.

The parser-free public carrier supports already charged raw byte buffers and
consuming slices. SQL-enabled hosts use the owning `apply_byte_string`,
`observe_byte_operand` and `publish_byte_operand` evaluator methods. The borrowed
`text::apply*`/TextResult API does not produce raw byte strings; it explicitly
rejects these producers. An ANSI conversion through StrConv remains a distinct
operation and must not silently change Unicode inputs to an inferred code page.

## Binary byte search

InStrB now accepts two strings, an optional leading byte start and an optional
fourth compare argument. Its [Microsoft specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/33e81067-4b43-4c45-a501-0acf1616f2f9)
distinguishes byte positions from InStr's character positions. Both Unicode Text
and lossless intermediates from ChrB/slicing/CVar are accepted. Binary arrays do
not become strings. Operands evaluate eagerly in written order, before start and
mode validation; NULL strings propagate after numeric argument validation.

Explicit compare 0 performs exact binary byte search. Omitted compare or -1
requires a host Option Compare policy for nontrivial searches; only Binary is
currently executable. Known text/database modes and positive LCID requests fail
as unsupported, with no linguistic-to-binary fallback. Invalid negative modes,
nonpositive starts and NULL numeric arguments have typed failures. The default
start is 1. The existing InStr shape rules also apply, including omitted start in
a three-slot call and a trailing missing compare slot; required string slots
cannot be omitted. LCID and native NULL/error precedence remain unqualified.

Empty haystacks or starts beyond the byte length return 0. Otherwise an empty
needle returns the start; a missing match returns 0. Nonempty search checks work
and cancellation for each candidate and compared byte. The public parser-free
`find_binary` kernel scans borrowed byte strings without output allocation;
evaluator encoding uses the same charged UTF-16LE buffers as slicing. There is
no raw Unicode decoding during matching.

```sh
tablecraft query eval "SELECT InStrB(1,'A中🦀','🦀',0),InStrB(1,'A',ChrB(0),0)"
```

Results are 5 and 2. Text/database/LCID byte-search providers, complete Variant
coercion and actual Access execution remain unfinished, so InStrB is **Partial**.

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
