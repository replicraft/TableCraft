# Explicit Japanese string encoding

`StrConv(...,64,1041)` and `StrConv(...,128,1041)` use a complete Japanese string
codec alongside the existing standalone Japanese width/kana conversions. The
call-local language is `ja-JP`; existing casing flags can combine with encoding,
with decoding before casing and encoding after it. No operating-system locale
or automatic ANSI page is read.

```sql
SELECT StrConv(StrConv('日本',128,1041),64,1041) AS Label,
       LenB(StrConv('日本',128,1041)) AS EncodedBytes;
```

The results are `日本` and `4`. The encoded bytes are `93 FA 96 7B`.

## Complete string policies

`functions::text::strconv::StringCodePage::from_windows_id(932)` selects the
existing dependency's `encoding_rs::SHIFT_JIS` codec. Its `encode` and `decode`
methods write through the existing bounded `ByteOutput` and `TextOutput` sinks.
The same type can wrap the previously supported single-byte pages.
`StringLocaleProfile` supplies a complete language/string-page pair, reusing the
existing fourteen locale identities and adding exact Japanese LCID 1041.

These are string operations. The existing `CharacterCodePage` policy retains its
one-byte contract and still refuses 932. Numeric `Chr`, `Asc` and `String` do not
gain packed DBCS behavior. Existing `LocaleProfile` and single-byte kernel APIs
also keep their original supported identities; callers needing Japanese strings
use the complete APIs. No new CLI page option or execution setting is introduced.

Microsoft's [Access StrConv reference](https://support.microsoft.com/en-us/access/functions/strconv-function)
establishes query availability, conversion settings and the optional LCID.
The [Windows identifier table](https://learn.microsoft.com/en-us/windows/win32/intl/code-page-identifiers)
identifies 932 as Japanese Shift-JIS. The implemented mapping contract remains
WHATWG/encoding_rs with strict Unicode round-trip admission; these references do
not establish native Access or Windows NLS equivalence.

## Exact mappings and known differences

Decoding uses the complete one/two-byte token and never inserts replacement text.
Dangling leads, invalid trails and undefined tokens refuse. ASCII, NUL, halfwidth
kana, Japanese syllabaries, kanji and qualified extension mappings use the
[WHATWG Shift_JIS decoder](https://encoding.spec.whatwg.org/#shift_jis-decoder).

Encoding requires both a successful non-replacing codec operation and exact
Unicode equality after decoding its result. This second check refuses yen,
overline and Unicode minus aliases: the underlying encoder accepts them, but the
decoded characters would be backslash, tilde and fullwidth minus. Astral scalars
and other unmappable characters also refuse; no best-fit output is substituted.

Duplicate byte spellings can decode to the same scalar. Both `EE EF` and `FA 40`
decode to U+2170; canonical encoding uses `FA 40`, following the
[WHATWG encoder](https://encoding.spec.whatwg.org/#shift_jis-encoder). Python's
CP932 encoder prefers another spelling, so it is not an authoritative reverse
mapping oracle.

The Microsoft-authored [CP932 mapping table](https://unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP932.TXT)
is a historical bytes-to-Unicode table, version 2.01 dated April 1998. The WHATWG
codec additionally decodes byte `80` as U+0080 and EUDC pairs beginning at `F0 40`
as private-use scalars. The current encoder refuses these EUDC Unicode inputs.
Standalone `A0`, `FD`, `FE` and `FF` refuse, unlike some Python/Windows codec
variants. The historical table neither defines modern native error behavior nor
chooses a canonical reverse spelling. Broader CP932, EUDC encoding, best-fit and
native default/error behavior remain explicit compatibility work.

## Retained values, errors and controls

Encoding 128 returns retained String bytes rather than stored Binary. `LenB`,
`AscB`, byte slicing and `CVar` can observe them before strict UTF-16 publication.
An odd encoded result remains usable as bytes but refuses direct final scalar
publication. Explicit conversion 64 interprets those bytes as Japanese text.
Ordinary Text passed to 64 still contributes its UTF-16LE bytes, and explicit
Binary input remains accepted there. Only the active retained byte range is read.

Flags and locale controls keep their existing checked conversions and eager
operand order. NULL sources propagate after valid controls. Unicode input to
128 is strictly published before encoding, preserving isolated-surrogate and
odd UTF-16 refusal. No malformed suffix, missing policy or failing operand is
hidden by a codec shortcut.

Every character/token checks work and cancellation; appends charge actual output
bytes before fallible allocation. Codec workspace uses fixed stack buffers, with
no entire-input unmetered conversion or table allocation. Failed cells return no
partial value. Query/CLI publication retains its existing buffered or explicitly
streaming contract. The implementation introduces no new dependencies and copies
no mapping tables into production.

## Verification boundary

Focused kernel and query controls cover complete public APIs, independent mapping
goldens, single-byte compatibility, raw/Binary inputs, grouping/correlation,
call-local policy reuse, eager failures and resource/cancellation boundaries.
Exhaustive independent mapping evidence uses separately fetched primary reference
files and a model of their published pointer rules. A successful engineering
comparison is separate from actual Microsoft Access execution qualification.

The private independent references were fetched read-only from the primary URLs
above. The Microsoft table's SHA-256 is
`c9bc0b0cd42e0fbcb82a09635bb5abed86afbdd4abc9e76fa5716638217cb59f`.
The [WHATWG JIS0208 index](https://encoding.spec.whatwg.org/index-jis0208.txt),
dated 2024-09-18, has SHA-256
`341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468`.
The [Unicode data license](https://www.unicode.org/license.txt) and
[WHATWG licensing terms](https://encoding.spec.whatwg.org/#ipr) were reviewed.
Reference files remain private, ignored test inputs: no mapping table or
third-party implementation is redistributed in the repository or embedded in
production. The independently authored model reads those supplied files; the
runtime uses the existing licensed `encoding_rs` dependency.

The scoped library/query check record is
[recorded separately](../../testing/results/read-query-japanese-string-encoding-2026-10-25.json).
