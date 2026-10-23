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
reinterpret Binary arrays as strings or implement VBA variable/UDT storage-size
overloads. The contextual evaluator now accepts scalar Len/LenB and AscB
sources through the explicit CStr routes below. Borrowed
policy-free observation kernels retain their Text-only admission. Full Variant coercion and native platform/error behavior remain
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
Positional arguments now use checked numeric/legacy DATE-to-Long conversion;
SQL numeric Text positions use the execution's explicit NumberFormat. Negative lengths,
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

## Numeric byte arguments

ChrB codes, LeftB/RightB/MidB counts/starts and InStrB starts now accept raw legacy
DATE and explicit-policy SQL numeric Text through checked half-even Long
conversion. InStrB mode Text likewise normalizes before its existing closed mode/
provider checks. Numeric validation still precedes NULL string propagation. The
current raw-byte/ordinary Text publication boundary and input/output budgets are
unchanged; no implicit Binary array or arbitrary source conversion is supplied.

The [mode/code batch](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
covers retained raw codes, rounded byte counts, byte matches, mode refusals and
inert native-field integration. Unsupported LCID/text/database byte-search
providers fail explicitly, with no binary fallback. [StrConv flags and LCIDs](STRCONV.md#contextual-flag-and-lcid-conversion)
now use contextual Integer/Long conversion in their owning evaluator, retaining
their separate supported flag/locale and representation profiles. Native Access
coercion, NULL/error precedence and broader raw-string consumers remain open.


## Scalar byte-string sources

The contextual evaluator now accepts ordinary scalar string arguments for LeftB,
RightB, MidB, AscB, InStrB and StrConv through the shared CStr service. Numeric
sources require the execution's NumberFormat; legacy dates require its DateTextFormat.
Boolean spelling, exact integer/fixed-point conversion and unsupported carriers
retain that service's existing policies. No OS locale or ANSI page is inferred.

After conversion, byte positions and observations still use UTF-16LE. For example,
`LeftB(1234,4)` returns Text `12`, and `InStrB(1,1234,23,0)` returns 3 under an
explicit numeric format. A raw `LeftB(1234,1)` remains unpublishable Unicode;
`AscB(LeftB(1234,1))` can observe its byte 49. Raw byte intermediates bypass CStr
and retain their exact bytes. Binary arrays remain separate and are accepted only
by StrConv's existing ANSI-array decoding path, not general byte-string calls.

Operands evaluate eagerly before option conversion. Existing position/flag/LCID
validation and NULL propagation precede source coercion where they did before.
CStr allocations, UTF-16 buffers, casing/encoding and publication share the same
cumulative byte/work/cancellation controls; per-value Text ceilings still apply.
Variable/UDT storage-size overloads and arbitrary expression-state/raw-string
consumers remain unfinished. Borrowed policy-free text APIs are unchanged.

Tests cover format rebinding, nested calls, strict publication, retained Binary
identity, NULL/eager errors and every observed conversion/encoding cancellation
checkpoint. The original [runner](testing/compare_byte_scalar_sources.py) passes
356 independent spelling/UTF-16 controls using both CLI query modes and raw
Jackcess getters from the unchanged registered priority-family native fixtures.
These checks establish the explicit conversion profile, not native Access SQL,
code-page equivalence or complete Variant coercion.


## Scalar length expressions

The contextual evaluator now routes ordinary scalar `Len`/`LenB` string-expression
sources through the shared CStr service. Boolean, integer, floating, Currency and
Decimal values use their existing spellings; numerics require an explicit
NumberFormat. Legacy DATE requires DateTextFormat. Text is borrowed and NULL
propagates without a formatting provider. Native-only GUID/extended DATE, Binary,
arrays and expression-only Error/Missing/Empty do not acquire implicit spellings.
The policy-free borrowed text kernels retain their Text/NULL admission.

[Microsoft's Access Len reference](https://support.microsoft.com/en-gb/access/functions/len-function)
and the [Len/LenB language contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/a4d9985a-e2cb-489c-a20a-5da09d77a59e)
distinguish string expressions/Variant string observations from variable storage
sizes. This implementation extends the explicit query string-expression profile;
it does not choose VBA variable/UDT storage sizes from a stored field's byte width
or establish native ACE overload selection for every query expression.

For example, with `--number-format en-us`, `Len(12345)` returns 5 and
`LenB(12345)` returns 10. Both observe the converted spelling, including a minus
sign, decimal separator or formatted date. Unicode lengths count UTF-16 units;
LenB doubles that count. Raw byte intermediates still bypass Unicode conversion:
`LenB(LeftB('A',1))` returns 1 while publishing that slice through ordinary Len
fails with UnrepresentableText. CVar, parameters and correlated subqueries use the
same request-local conversion policies; prepared plans never cache a spelling.

Conversion output and the subsequent scan share allocation/work/cancellation and
per-Text limits, even though the final Long is small. A source expression failure
propagates before conversion. Tests check every observed conversion/scan
cancellation point, format absence, NULL, Binary refusal, wrong arity and strict
raw-byte publication. The independent runner above now includes Len/LenB and CVar
observations alongside byte operations in both CLI modes, plus one-to-five-digit
native Long fields through sequential/ascending/descending reads in four families.
Only TableCraft executes SQL; the independent models and raw Jackcess getters do
not qualify native Access query coercion, locale or overload behavior.

## State-aware StrConv input

`ExpressionByteOperand` now retains scalar expression states or moves an existing
raw ByteString into [StrConv admission](STRCONV.md#ordered-expression-state-admission).
Flags and LCID validate before the source is admitted, while written source and
control operations stay eager. CVar and nested raw producers retain their
representation; the output remains the original ByteOperand type with strict
ordinary Unicode publication. This supplies no new Error storage, inspection or
state behavior for other byte functions.
