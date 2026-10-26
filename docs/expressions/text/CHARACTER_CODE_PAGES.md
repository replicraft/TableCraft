# Explicit numeric character code pages

Numeric character construction now accepts a validated execution-local
`CharacterCodePage`. Consumers include `String(number, character)`, [Chr and Asc](CHARACTER_FUNCTIONS.md).
[StrConv](STRCONV.md) also uses these pages for explicit Unicode/single-byte
string conversion. Explicit Japanese StrConv instead uses its separate complete
[string codec](JAPANESE_STRING_ENCODING.md), without changing this character API.
For String, a numeric character operand is checked/rounded to Int32, reduced modulo 256,
then decoded using the selected page before bounded repetition. Chr enforces a
byte range without modulo; Asc uses exact non-replacing single-byte encoding.

## Supported pages and mapping contract

| Windows/OEM identifier | Selected encoding_rs mapping |
| --- | --- |
| 866 | IBM866 (OEM Cyrillic) |
| 874 | Windows Thai |
| 1250 | Windows Central European |
| 1251 | Windows Cyrillic |
| 1252 | Windows Western European |
| 1253 | Windows Greek |
| 1254 | Windows Turkish |
| 1255 | Windows Hebrew |
| 1256 | Windows Arabic |
| 1257 | Windows Baltic |
| 1258 | Windows Vietnamese |

These are explicit single-byte profiles, not automatic system/ANSI aliases.
Unknown IDs, ACP/OEM defaults, multibyte pages such as 932, and Unicode encodings
are UnsupportedCharacterCodePage. With no selection, numeric bytes 0–127 retain
the existing ASCII profile and high bytes return CharacterPolicyRequired.

The implementation uses `encoding_rs` 0.8.42 with defaults disabled. One byte is
passed to a fresh decoder without BOM handling or replacement, using a fixed
stack UTF-16 buffer. The decoder must consume exactly one byte and emit exactly
one representable BMP character; unmapped bytes return InvalidCharacterCode.
No replacement character, alias guessing, locale lookup or heap decode buffer is
introduced. Typed page values have a private ID and a validating constructor;
upstream encoding types do not cross TableCraft's API boundary.

**The mapping contract is WHATWG/encoding_rs, not observed Windows Access behavior.**
In particular, formerly undefined C1 entries can map to C1 controls, and Windows-
1255 byte CA maps to U+05BA under the reviewed WHATWG index. Other unmapped slots,
such as Greek page 1253 byte AA, fail. These distinctions remain explicit native
qualification work; a successful library decode is not an ACE conformance result.

## Library and CLI controls

- `CharacterCodePage::from_windows_id` validates a page; `windows_id` exposes its ID.
- `functions::text::apply_with_context` accepts `TextContext::character_code_page`.
  Existing `apply`/`apply_with_collation` entry points keep their prior defaults.
- `Evaluator::with_character_code_page` selects the scalar execution policy.
- `ExecutionOptions::character_code_page` forwards it to all query evaluators,
  including nested/grouped calls and named-source execution. Preparation does not
  capture this setting; reused plans can change pages and return to omission.
- Existing SQLite workers forward `AsyncOptions::character_code_page` per request.
  This maintains the options boundary without expanding SQLite or enabling a CLI
  async runtime.

```sh
tablecraft query eval "SELECT String(2,192) AS Label" --character-code-page 1252
tablecraft query run "SELECT String(2,192) AS Label" --character-code-page 1251
```

The respective Text values are `ÀÀ` and `АА`. Both query commands validate the flag
before database I/O. Unsupported IDs produce an argument error; output remains
buffered on runtime mapping/budget failures. Parse and native inspection commands
do not accept this execution setting and never execute calls to discover a page.

The policy affects numeric character codes and explicit StrConv encoding calls.
It does not transcode ordinary Text arguments to other functions, select SQL
collation, change native field decoding/index weights,
read database LCIDs, or implement VBA execution. Construction retains its existing
NULL/count/surrogate and [resource contracts](TEXT_CONSTRUCTION.md).

## References and evidence

Microsoft documents [numeric String character codes](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/string-function)
and [Windows code-page identifiers](https://learn.microsoft.com/en-us/windows/win32/intl/code-page-identifiers).
The [encoding_rs documentation](https://docs.rs/encoding_rs/0.8.42/encoding_rs/)
explains its WHATWG coverage and Windows differences. The reviewed primary
[1252](https://encoding.spec.whatwg.org/index-windows-1252.txt),
[1251](https://encoding.spec.whatwg.org/index-windows-1251.txt),
[1253](https://encoding.spec.whatwg.org/index-windows-1253.txt) and
[1255](https://encoding.spec.whatwg.org/index-windows-1255.txt) indexes establish
specific control, unmapped-byte and Hebrew mapping cases. No index files or
third-party fixtures are bundled by this feature.

Two decoder tests cover independent mapping goldens, invalid IDs/unmapped bytes,
and every ASCII byte across all supported pages. An evaluator test, two query
tests, two CLI tests and a worker test cover context/plan/request isolation,
modulo behavior, unchanged Text, nested/grouped calls, inert commands and buffered
errors. The [independent recipe](../../testing/compare_character_code_pages.py) exhausts
all 256 bytes in all eleven pages through both CLI query paths. It uses Python
codecs with documented WHATWG C1/Hebrew extensions, checks unmapped-byte refusals,
and compares sequential/indexed native projections while preserving source bytes.
[Implementation status](../../IMPLEMENTATION.md) records totals and validation scope.

This extends `String`'s Partial implementation in the
[built-in tracker](../ACCESS_BUILTINS.md). Full Variant/DBCS/Windows mapping semantics,
native default/error behavior and actual Access comparisons remain incomplete.
