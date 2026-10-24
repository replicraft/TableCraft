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

## Complete UTF-16 unit observations

Len and AscW now consume lossless intermediates containing complete UTF-16LE
units. Len returns the unit count as Long; AscW returns the first unit as signed
Integer, including an unpaired surrogate. Neither operation decodes the entire
string into Rust Text, substitutes a replacement character or changes stored data.
Odd byte counts remain UnrepresentableText for these Unicode observations;
LenB/AscB retain their separate byte-level behavior. An empty raw string has Len
zero, while AscW still fails Domain on an empty string.

ChrW now also supplies a two-byte intermediate to nested byte consumers and
inspection. Its existing checked Long, -32768..65535 code-unit range, signed
spelling and explicit numeric Text/DATE conversion rules are shared with the
strict character kernel. CVar retains that intermediate, and TypeName/VarType
continue to report String/8. No Binary, array or stored carrier is introduced.

```sh
tablecraft query eval "SELECT Len(ChrW(55296)),AscW(ChrW(55296)),LenB(ChrW(55296))"
tablecraft query run "SELECT Len(LeftB('😀',2)),AscW(RightB('😀',2))"
```

These return `1, -10240, 2` and `1, -8704` respectively. Direct `ChrW(55296)`
publication still fails UnrepresentableText. Arbitrary
operators/selection, source parameters and relational string cells retain their
existing representable-Text boundaries. The borrowed scalar ChrW API likewise
keeps its strict character/publication contract.

The existing [Len/LenB specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/a4d9985a-e2cb-489c-a20a-5da09d77a59e)
and [Chr/ChrW](https://support.microsoft.com/en-us/access/functions/chr-function)
and [Asc/AscW](https://support.microsoft.com/en-us/access/functions/asc-function)
contracts motivate this explicit Windows-style UTF-16 profile; native Access
surrogate/odd-byte query behavior remains unqualified. Both query hosts reuse the
same evaluator; raw storage is charged before allocation, observations require
no new payload allocation and cancellation remains active.

The expanded [independent byte recipe](testing/compare_byte_strings.py) compares
26,590 source-free values/refusals against Python UTF-16LE bytes. It includes
every surrogate unit, signed code spellings, partial-byte slices, inspection and
both CLI query paths. It executes no native Access SQL or application code.

## Nested UTF-16 unit slices

Left/Right/Mid now retain whole UTF-16 units when consumed through the lossless
string path by Len/LenB, AscW/AscB, byte producers, CVar or type inspection.
For example, `AscW(Left('😀',1))` returns -10179 and
`AscW(Right('😀',1))` returns -8704. A nested slice can consume another raw
slice or ChrW surrogate without decoding it. Odd-byte inputs remain refused;
the evaluator does not invent an alignment or drop bytes.

Microsoft's [Left query example and count contract](https://support.microsoft.com/en-us/access/functions/left-function)
and [Mid query examples and optional length](https://support.microsoft.com/en-us/access/functions/mid-function)
establish direct query availability and distinguish character counts from byte
counts. This implementation uses TableCraft's explicit Windows-style UTF-16
profile; surrogate behavior has not been compared with native Access.

Operands evaluate eagerly before admission. Source Error/Missing and explicit
CStr coercion precede positional conversion; a NULL source propagates before
admitting controls, but cannot suppress an operand's operational failure.
Mid validates its positive start before admitting an optional length. Counts use
the existing checked Long rules. Huge requests clamp in units before converting
to byte offsets. Existing raw buffers slice without another payload allocation;
Text sources encode under cumulative allocation, work and cancellation controls.

Ordinary direct slices with materialized scalar sources retain the existing
borrowed path and its budget behavior. When the source is a raw-producing call,
both query hosts retain its units through the outer slice before publishing.
Thus `Left(ChrW(55296),0)` returns empty Text, and
`Mid(Left('A😀Z',3),2,2)` publishes the complete supplementary character. The
inner source does not have to become a scalar cell first. Direct publication of
a surrogate-splitting final slice still fails, and
raw intermediates cannot cross relational cells or arbitrary scalar consumers.
The independent Python byte recipe now also checks ordinary UTF-16 slices,
including supplementary characters, omitted lengths and maximum Long controls.

## Remaining byte-string work

[Ampersand concatenation](CONCATENATION.md#lossless-utf-16-concatenation) now joins
complete raw units, including surrogate halves that form valid final Text. Its
nested observations and inspection retain String identity; odd-byte concatenation
remains explicitly refused. Other operators retain the limits below.

[StrConv](STRCONV.md) now supports explicit casing and strict single-byte
Unicode/ANSI conversions; width/kana and broader mapping behavior remain open.
Its encoding results use these same lossless byte-string intermediates. InStrB
supports the explicit binary profile below. The current lossless paths do not yet
extend to arbitrary conditional/operator arguments, odd-byte Unicode observations,
array elements, parameter/result carriers, derived-table cells or raw-string
comparison/sorting. Those boundaries still require ordinary representable Text;
they do not silently replace or truncate invalid data. Dollar-suffixed aliases,
full Variant coercion and native Access behavior remain unqualified or unsupported.
The four producers therefore remain **Partial**, alongside LenB/AscB.

The parser-free public carrier supports already charged raw byte buffers and
consuming slices. SQL-enabled hosts use the owning `apply_byte_string`,
`observe_byte_operand` and `publish_byte_operand` evaluator methods. The borrowed
`text::apply*`/TextResult API does not produce raw byte strings; it explicitly
rejects the byte-only producers; ChrW retains its existing strict scalar result.
An ANSI conversion through StrConv remains a distinct
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
arrays remain refused. Expression states now use the dedicated admission profile
below: host Empty has a string target, while Error/Missing do not acquire
implicit spellings.
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
ordinary Unicode publication. This supplies no new Error storage or broad relational raw-string
carrier; the other byte consumers now share the explicit admission profile below.

## Ordered byte-function expression states

LenB, AscB, ChrB, LeftB, RightB, MidB and InStrB now retain eager expression states
through their owning admission points. They reuse ExpressionByteOperand and the
original borrowed/view/encoding/search kernels. Materialized public entry points
remain available; state-aware producers, observations and search add no stored
Error/Empty/Missing or broader raw relational carrier.

Host Empty selects empty Text for source strings and numeric zero for supplied
codes/controls. Thus LenB(Empty) is zero, AscB(Empty) fails for empty input and
ChrB(Empty) produces the raw byte zero. LeftB/RightB count zero selects an empty
view; MidB start zero still fails. These are host API states, not a new SQL
constructor; Nz(NULL) still supplies ordinary empty Text. Error data requires
explicit conversion and Missing gives NonScalarExpression when reached.

Slices convert position and any supplied length before checking their combined
ranges and admitting the source. In particular, `MidB(CVErr(11),0,CVErr(12))`
reports TypeMismatch during length admission; a negative validly converted
position fails before source Error admission. Source NULL still propagates after
valid controls. These preserved byte-slice stages differ from text Mid's profile.

InStrB validates comparison mode, start and the source NULL gate, then admits
and formats each source progressively from left to right. NULL can mask retained
Error/Missing source data after valid controls; operational errors in written
arguments remain eager. A numeric left source without NumberFormat still fails
that policy before admitting right-hand Error data. Empty-source/needle early
results occur only after both source conversions. Text/database/LCID byte-search
providers remain unavailable; a mode above the standard range is refused before
source admission, with no binary fallback.

The same preparation services serve materialized/state callers. Raw slice views
move their existing buffers and ranges, including odd/unpaired units, under the
same cumulative controls. Ordinary Unicode publication remains strict. Byte
observations and search never force publication or reinterpret raw strings as
stored Binary. No return category, code page, provider, alias or dependency is added.

The existing [ChrB contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/900323ac-8160-42cd-ae52-4073f2a95116),
[Mid/MidB contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/4a65ee4e-b6b9-45d3-a3f6-576fed4bb227)
and [InStr/InStrB contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/33e81067-4b43-4c45-a501-0acf1616f2f9)
guide byte roles, ranges, NULL and result positions. The shared
[conversion basis](STRCONV.md#ordered-expression-state-admission) separates
Empty targets and explicit Error conversion. Native Access precedence and full
Variant/raw-string semantics remain unqualified.

Four expression tests cover all seven functions, Empty/code/source distinctions,
control order, NULL masking, progressive source policy failures, raw view buffer
identity under zero additional byte allowance, every observed successful slice/
search cancellation checkpoint and once-only Rnd controls. Query/CLI tests add
derived states, explicit conversion and atomic failures. The new
[runner](testing/compare_byte_states.py) passes 364 controls (300 scalar and 64
four-family sequential/indexed cases) over unchanged registered inputs. The
existing byte runner additionally passes 3,792 scalar controls; its optional
native branch was not rerun. Only TableCraft executes SQL. Full Variant, broader
raw carriers, provider and Windows/native Access qualification remain open.
