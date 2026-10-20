# Chr, Asc, ChrW and AscW

These four unqualified calls now share the text-function kernel in scalar SELECTs
and bound retrieval, including grouping, nested expressions and reusable parameters.
They do not invoke VBA. [Character-code policies](CHARACTER_CODE_PAGES.md) remain
execution-local; preparation does not capture a page.

| Function | Current behavior |
| --- | --- |
| Chr(code) | Checked half-even numeric-to-Int32 conversion followed by the single-byte range 0–255. Decodes using the selected page; without one, only ASCII is accepted. Unlike String, this function does not reduce out-of-range codes modulo 256. |
| Asc(text) | Encodes the first character in the selected single-byte page and returns its byte code as Int16 (0–255). Without a page, only ASCII is accepted. Empty Text fails. |
| ChrW(code) | Checked numeric-to-Int32 conversion, accepting signed Int16 or unsigned UTF-16 spellings (-32768–65535). Negative spellings reinterpret the 16-bit unit; surrogate units cannot become stored Text and fail. |
| AscW(text) | Returns the first UTF-16 unit as a signed Int16. This includes a leading surrogate from supplementary input, without modifying or constructing an unpaired-surrogate string. Empty Text fails. |

All four reject NULL with InvalidNullConversion. Asc/AscW currently accept Text
only; numeric-to-string conversion is absent. Chr/ChrW accept the existing explicit
numeric/Boolean conversion profile; locale-sensitive numeric Text is absent.
Wrong arities, missing slots, wildcards and qualifiers fail before operands run.
Written expressions remain eager, including calls in unselected IIf branches.
Native coercion/result/error precedence still requires Access qualification.

ChrW/AscW use the explicit Windows-style UTF-16 profile on every platform, without
emulating historical Macintosh VBA guesses. For example, ChrW(-1) produces U+FFFF,
AscW(U+FFFF) returns -1 and AscW of a leading U+1F600 returns -10179 (unit D83D).
ChrW of D83D fails with UnrepresentableText; supplementary scalar construction is
not inferred from a number greater than 65535.

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

The [independent recipe](testing/compare_character_functions.py) builds inverse
mappings from Python codecs and documented WHATWG extensions, then compares Chr
and Asc results across all eleven pages through both CLI query paths. It separately
checks Unicode units, refused boundaries, supplementary input and independently
decoded sequential/indexed native rows, preserving original database bytes.
[Implementation status](IMPLEMENTATION.md) records counts and check scope.

All four are Partial in the [built-in tracker](ACCESS_BUILTINS.md). Full Variant
coercion, DBCS/native mappings and error/default behavior, lossless surrogate
results and Windows Access execution remain incomplete. AscB/ChrB and `$` aliases
remain separate unsupported calls.
