# Chr, Asc, ChrW and AscW

These four unqualified calls now share the text-function kernel in scalar SELECTs
and bound retrieval, including grouping, nested expressions and reusable parameters.
They do not invoke VBA. [Character-code policies](CHARACTER_CODE_PAGES.md) remain
execution-local; preparation does not capture a page.

| Function | Current behavior |
| --- | --- |
| Chr(code) | Checked half-even numeric-to-Int32 conversion followed by the single-byte range 0–255. Decodes using the selected page; without one, only ASCII is accepted. Unlike String, this function does not reduce out-of-range codes modulo 256. |
| Asc(text) | Encodes the first character in the selected single-byte page and returns its byte code as Int16 (0–255). Without a page, only ASCII is accepted. Empty Text fails. |
| ChrW(code) | Checked numeric-to-Int32 conversion, accepting signed Int16 or unsigned UTF-16 spellings (-32768–65535). Negative spellings reinterpret the unit; nested byte consumers can retain it, while surrogate units cannot publish as stored Text. |
| AscW(text) | Returns the first UTF-16 unit as a signed Int16. This includes a leading surrogate from supplementary input, without modifying or constructing an unpaired-surrogate string. Empty Text fails. |

All four reject NULL with InvalidNullConversion. SQL Asc/AscW now convert ordinary
scalar string operands through explicit CStr policies; their pure kernels remain
Text-only. See [scalar sources](#scalar-string-sources). Chr/ChrW accept the existing explicit
numeric/Boolean and legacy DATE Long conversion profile. SQL numeric Text codes
use the explicitly selected NumberFormat; no ambient locale is consulted.
Wrong arities, missing slots, wildcards and qualifiers fail before operands run.
Written expressions remain eager, including calls in unselected IIf branches.
Native coercion/result/error precedence still requires Access qualification.

ChrW/AscW use the explicit Windows-style UTF-16 profile on every platform, without
emulating historical Macintosh VBA guesses. For example, ChrW(-1) produces U+FFFF,
AscW(U+FFFF) returns -1 and AscW of a leading U+1F600 returns -10179 (unit D83D).
Direct ChrW of D83D publication fails with UnrepresentableText; supplementary scalar construction is
not inferred from a number greater than 65535.
Nested [raw UTF-16 observations](BYTE_STRING_FUNCTIONS.md#complete-utf-16-unit-observations)
now retain individual surrogate units through Len, LenB, AscW, AscB, byte slices,
explicit binary byte search and type inspection without relaxing publication.

## Encoding and resources

`CharacterCodePage::encode_character` uses encoding_rs's non-replacing UTF-16
encoder with fixed stack buffers. It requires one output byte and an exact decoded
round trip, preventing best-fit aliases or replacement text. Unmappable input is
UnmappableCharacter; selected-page decode holes remain InvalidCharacterCode.
The [WHATWG/native mapping distinction](CHARACTER_CODE_PAGES.md) still applies.
DBCS byte packing, best-fit Windows conversions and byte-oriented AscB/ChrB are
not implemented by these single-byte functions.

The kernel allocates no intermediate String. Character results materialize through
the evaluator's existing text helper, charging UTF-8 bytes before fallible allocation;
code results retain an exact Int16 carrier. Normal input-size, depth, work and
cancellation checks apply. Complete query/CLI output remains buffered on failure.

```sh
tablecraft query eval "SELECT Chr(65), Asc('Apple'), ChrW(8364), AscW('😀')"
tablecraft query run "SELECT Chr(128), Asc('€')" --character-code-page 1252
```

The first returns Text A, Integer 65, Text € and Integer -10179; the second returns
Text € and Integer 128. The flag does not affect ChrW/AscW or native field decoding.

## References, evidence and remaining work

Microsoft's [Chr/ChrW reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/chr-function)
and [Asc/AscW reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/asc-function)
guide character construction, first-character codes, Integer results, non-DBCS
ranges and the distinct Unicode/byte variants. The implementation's signed-unit,
NULL and exact-encoding policies above are explicit contracts, not observed native
Access behavior.

Four character-kernel tests cover byte-page examples, signed/unsigned spellings,
surrogate and NULL/type/domain cases, half-even conversion, and every representable
BMP unit against an independent signed model. A separate encoder test checks every
mapped byte in all eleven selected pages and rejects unmappable text. An evaluator
test checks byte charging and cancellation; two query tests cover grouping, nesting,
parameter/page reuse and eager failures. A CLI test decodes CSV/TSV cells and checks
both query paths, page selection and buffered refusals.

The [independent recipe](../../testing/compare_character_functions.py) builds inverse
mappings from Python codecs and documented WHATWG extensions, then compares Chr
and Asc results across all eleven pages through both CLI query paths. It separately
checks Unicode units, refused boundaries, supplementary input and independently
decoded sequential/indexed native rows, preserving original database bytes.
[Implementation status](../../IMPLEMENTATION.md) records counts and check scope.

All four are Partial in the [built-in tracker](../ACCESS_BUILTINS.md). Full Variant
coercion, DBCS/native mappings and error/default behavior, general raw-string
results and Windows Access execution remain incomplete. AscB/ChrB and Chr$ retain
their separate documented contracts; wider typed aliases remain unsupported.

## Raw first-byte observation

[AscB](BYTE_STRING_FUNCTIONS.md) now reads the first UTF-16LE byte of Unicode
Text independently of these character-code page policies. It returns Integer;
ChrB and lossless byte-string construction remain unimplemented. Asc/Chr do not
become aliases for raw byte-string operations.

`Chr$` is now an explicit supported spelling of the same checked Chr profile,
including NULL rejection and code-page requirements. Microsoft's
[Eval reference](https://support.microsoft.com/en-au/access/functions/eval-function)
uses Chr$ in an expression string. This does not make other `$` names aliases or
qualify native character coercion/range/platform behavior.

## Contextual character-code conversion

Chr/ChrW now use the existing checked Long conversion for raw legacy DATE codes.
SQL numeric Text codes use the execution's number policy before the same range,
code-page and surrogate checks. For example, `Chr('65,5')` under de-de produces
B after half-even rounding. ChrW's signed UTF-16 profile remains independent of
the selected single-byte page. Numeric Text without a policy fails explicitly;
pure text kernels still reject Text codes.

ChrB, raw byte slicing positions and InStrB starts now use the same contextual
numeric/DATE conversion through the owning byte evaluator. Their raw bytes never
become stored Binary or malformed ordinary Text. String's Text character argument
remains a literal first-unit source; Asc/AscW operands are not reinterpreted as
numeric character codes. [StrConv options](STRCONV.md#contextual-flag-and-lcid-conversion)
now use their distinct Integer flag/Long LCID targets with the same explicit
numeric policy, while retaining their own locale/representation checks.

Normalization, streamed output and raw byte buffers use cumulative resource limits.
A control permits four normalized code bytes plus one result byte; a separate
three-byte control permits two normalized digits and one retained raw byte.
Every observed ChrW conversion/output checkpoint can cancel. Selected Windows-1252
still maps byte 128 to U+20AC; no-page high bytes, unpaired ChrW scalar publication and
out-of-range codes still fail. No DBCS, best-fit or platform locale discovery is
introduced. The [combined mode/code evidence](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
records current checks and native Access gaps; all character built-ins remain
Partial.


## Scalar string sources

SQL Asc/AscW use the shared CStr service for ordinary non-Text scalar sources.
Numeric values require NumberFormat, dates require DateTextFormat and Boolean
values retain the existing English spelling. Text remains borrowed. For example,
`Asc(65)` with `--number-format en-us` returns 54, the first character code of
"65", rather than treating 65 as a character-construction argument. Asc/AscW
results remain Int16. Empty Text, NULL and unsupported Binary/GUID retain their
explicit refusal boundaries. [Expression-state admission](CSTR.md#unary-string-expression-states)
now selects empty Text for host Empty and retains Error/Missing type refusals.

The converter materializes once and re-enters the existing character function
with proven Text. Existing code-page selection, first-character/signed-unit rules,
Unicode behavior and buffered query publication remain in force. Source conversion
shares cumulative bytes/work, per-value Text and cancellation controls; no
formatted value is retained between prepared executions.

Microsoft's [Access Asc reference](https://support.microsoft.com/en-au/access/functions/asc-function)
and [String coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
provide the string-argument and conversion basis. This is TableCraft's bounded
materialized scalar profile; full Variant/native regional/error behavior, DBCS
and native Access code-page/Unicode qualification remain open.

The original [scalar runner](../../testing/compare_character_reverse_scalars.py) passes
336 controls over authored numeric/date/Boolean spellings, first ASCII codes, typed results and BMP
reversal. Independent raw IDs supply native-source controls through sequential
and both index orders over the unchanged four-family 20,000-row inputs. Only
TableCraft evaluates SQL; those getters do not establish native Access semantics.

## Character construction expression states

Chr, Chr$ and ChrW now retain their eager source as expression data until checked
character-code admission. Host Empty selects numeric zero and produces NUL.
Error data gives TypeMismatch; Missing gives NonScalarExpression. Explicit
`Chr(CLng(CVErr(65)))` still produces `A`, while `Chr(CVErr(65))` does not
implicitly use that code. NULL remains InvalidNullConversion.

The evaluator borrows scalar inputs and reuses checked Long conversion, numeric
Text policies and the original code-page/UTF-16/output kernels. Chr$ keeps its
existing Scalar String declaration; no byte-specific ChrB behavior changes.
Surrogate, range, code-page, cumulative resource and cancellation guards remain.
The [Access Chr contract](https://support.microsoft.com/en-us/access/functions/chr-function),
[numeric Empty conversion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e2637723-3010-449d-bead-9f17533dc105)
and [explicit Error conversion distinction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/cverr-function)
guide this existing query-function profile. Native Access coercion remains unrun.

Two expression tests exercise host states, exact conversion/output accounting and
every observed successful cancellation checkpoint. Query/CLI tests cover derived
Error values, explicit Long conversion and buffered failures. The shared
[independent character/format runner](../../testing/compare_character_format_states.py)
checks raw Error refusal, independent Windows-1252/UTF-16 code models and selected
native identities without reference SQL. See [numeric formatting states](../formatting/STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states)
for the combined evidence.
