# StrConv casing and string encoding

StrConv now executes in scalar and bound read-only queries. Microsoft's
[Access reference](https://support.microsoft.com/en-us/access/functions/strconv-function)
provides direct upper/lower query examples, numeric conversion settings and an
optional LCID. Its broader
[VBA reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strconv-function)
also describes converting ANSI byte arrays to strings. Implementing this closed
function does not execute stored VBA or activate application objects.

## Implemented conversion profile

| Conversion | Current behavior |
| --- | --- |
| 0 | Preserve the owned string representation; no-op native behavior remains unqualified |
| 1 / 2 | Upper/lowercase through the existing explicit ICU casing profile |
| 3 | Proper case using documented separators and explicit ICU segment titlecasing |
| 64 | Decode raw string/Byte-array bytes through an explicit single-byte page |
| 128 | Encode representable Unicode as a lossless single-byte string intermediate |
| Case setting plus 64 or 128 | Decode before casing; case before encoding |

Conversion is checked/rounded to Integer; an explicit LCID is checked/rounded to
Long and structurally validated without dropping alternate-sort bits. Both slots
now accept raw legacy DATE and execution-local numeric Text conversion. The
[MS-VBAL declaration](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/17afbc5b-e1c7-4717-bfe3-9ae938ef0736)
describes the flag argument and Long LocaleID; the bounded coercion targets here
retain their preexisting profile rather than claiming complete native enum/Variant
call semantics. Unknown
flag bits, negative flags and mutually exclusive settings fail. Width/kana
requests (4/8/16/32), even in combinations, report unsupported rather than
silently omitting the requested transformation. These modes remain implementation
work, not permanently excluded native features.

Arguments evaluate eagerly in written order, including the optional LCID, before
flag/locale validation. NULL strings propagate after that validation. Non-string
scalar carriers fail TypeMismatch. Binary is accepted specifically as a Byte-array
input to conversion 64; it is not generally aliased to Text. Full Variant
coercion, NULL/error precedence, zero/no-op flags and native conversion ordering
remain qualification work.

## Explicit language and code-page selection

Without an LCID, execution-local `case_locale` and `character_code_page` select
the existing [casing](TEXT_CASING.md) and [single-byte mapping](CHARACTER_CODE_PAGES.md)
profiles. Non-NULL casing requires a language. Encoding/decoding without a page
permits ASCII only; higher values need a selected page. Library code never reads
system settings to invent an ANSI page or language.

An explicit supported LCID overrides both settings for that call only. A reused
plan can change its LCID parameters or host policies without retaining a prior
selection. The current exact zero-sort profiles are:

| LCID | Casing language | Code page |
| ---: | --- | ---: |
| 1025 | ar-SA | 1256 |
| 1029 | cs-CZ | 1250 |
| 1031 | de-DE | 1252 |
| 1032 | el-GR | 1253 |
| 1033 | en-US | 1252 |
| 1036 | fr-FR | 1252 |
| 1037 | he-IL | 1255 |
| 1043 | nl-NL | 1252 |
| 1049 | ru-RU | 1251 |
| 1054 | th-TH | 874 |
| 1055 | tr-TR | 1254 |
| 1061 | et-EE | 1257 |
| 1063 | lt-LT | 1257 |
| 1066 | vi-VN | 1258 |

The language/page identities come from Microsoft's
[Windows SDK language table](https://learn.microsoft.com/en-us/windows/win32/msi/localizing-the-error-and-actiontext-tables).
Selecting ICU/encoding_rs for them is TableCraft's explicit provisional profile,
not evidence of Windows NLS equivalence. Unknown exact LCIDs, alternate sorts,
system/custom aliases and unsupported multibyte profiles fail without fallback.
LCID 0/reserved-bit identities fail structural validation. This bounded table
does not claim complete Windows locale coverage.

Proper casing splits only at NUL, tab, LF, VT, FF, CR and ASCII space, preserving
the separator characters. Each segment titlecases its first cased character and
lowercases the suffix through ICU, retaining contextual language behavior.
Apostrophes and hyphens do not introduce new segments. Leading uncased characters,
DBCS separators, Unicode titlecase expansions and native word-start behavior
remain unqualified. This profile is not generic whitespace splitting or Unicode
word segmentation.

## Representation and resource boundaries

Conversion 128 returns String bytes, not a stored Binary carrier. The
[lossless byte-string paths](BYTE_STRING_FUNCTIONS.md) let LenB/AscB, slicing,
InStrB, CVar and type inspection consume those bytes without UTF-16 publication.
Conversion 64 decodes every input byte, including zeros. When its input is ordinary
Text, it observes that Text's UTF-16LE bytes; it does not pretend the original
Unicode characters were already ANSI. For example, StrConv('A',64) yields A
followed by NUL under the ASCII profile.

```sh
tablecraft query eval "SELECT StrConv('hELLO WORLD',3,1033) AS Label"
tablecraft query run "SELECT StrConv(StrConv('é€',128,1033),64,1033) AS Label"
tablecraft query eval "SELECT AscB(StrConv('I',130,1055)),LenB(StrConv('é€',128,1033))"
```

The results are `Hello World`, `é€`, and numeric values 253 and 2. Publishing a
raw single-byte intermediate directly still requires valid UTF-16LE under the
existing scalar-cell boundary. An odd encoded result fails UnrepresentableText;
an even result can decode to different Unicode characters. Use an explicit
conversion 64 to interpret ANSI bytes as their intended text. Arbitrary raw-string
operators/result carriers are still unfinished.

Encoding uses exact round trips through the selected existing single-byte page.
Unmappable characters and unmapped bytes fail; no best-fit mapping, replacement
character or silent byte loss occurs. Native best-fit/replacement behavior and
full DBCS/Unicode code-page support remain compatibility work. Character-page
WHATWG/Windows differences stay explicit in the owning mapping document.

Parser-free hosts can use `Conversion`, `LocaleProfile`, borrowed casing recipes,
`encode_ansi` and `decode_ansi`. TextOutput/ByteOutput sinks own bounded, fallible
allocation and work/cancellation checks; discard prefixes on any failure. The
SQL-enabled evaluator's owning `apply_strconv` returns a scalar or byte operand.
The borrowed text::apply*/TextResult API explicitly rejects this owning operation.

Input Text and all materialized output retain the existing per-value ceilings.
Raw output charges bytes before fallible reserve; each source character/byte and
append checks work/cancellation. ICU uses the existing conservative context
preflight and metered output bridge. Casing/encoding failures never publish
partial CLI stdout. No new library, runtime dependency or stored Value variant
was introduced.

## Evidence and open work

Nine kernel/scalar controls cover flag conflicts, exact LCID identities, strict
mapping goldens, NUL/separators, a fixed-seed 256-case ASCII proper-case model,
nested bytes, result types, output-expansion ceilings, eager failures and every observed scalar cancellation
checkpoint. Three query controls cover field/correlated/aggregate/grouping
visitors, Binary inputs, parameter/host-policy reuse, call-local overrides and
every observed query cancellation checkpoint. Two CLI controls exercise both
query modes, explicit policy overrides and empty stdout on errors.

The original [comparison runner](testing/compare_strconv.py) passes 12178 controls:
Python codec mappings plus reviewed WHATWG exceptions exhaust all bytes of the
eleven existing pages; selected Python casing models and explicit language goldens
cover casing; original native Hyperlink fixtures supply independent decoded fields
in Jet 4/ACE 2007/2016/2019 files. All original hashes remain unchanged. Jackcess
supplies values without executing StrConv. Actual Microsoft Access evaluation,
full native error/coercion/NLS behavior, width/kana modes, more LCIDs, DBCS and
best-fit policies remain open, so StrConv stays **Partial**.

## Contextual flag and LCID conversion

The evaluator now reuses the checked numeric conversion service for both options:
Integer for flags and Long for LCIDs, including half-even fixed/IEEE rounding,
raw DATE serials and explicit NumberFormat Text normalization. No new conversion
algorithm or dependency is introduced. For example,
`StrConv('ABC','1,5','1033')` under de-de returns `abc`, and the equivalent DATE
options do not require a number profile. Text options without a profile fail
rather than infer regional settings.

Flag conversion and flag validation still precede LCID conversion/lookup, followed
by source NULL propagation and representation checks. Unsupported width/kana flags
and unknown LCIDs are never silently replaced by a supported setting. All written
argument expressions remain eager, so a late expression failure remains visible
before option conversion. NULL options retain InvalidNullConversion. No new LCID,
width/kana, DBCS, source carrier or default-locale implementation is supplied.

Both normalizations and casing/encoding output share cumulative limits. An exact
eight-byte control permits three flag bytes, four LCID bytes and a one-byte cased
result while moving the original input. Every observed conversion/casing/output
checkpoint is cancelled independently. Reused prepared plans retain execution-local
number policy and exact locale selection; encoding still preserves raw byte-string
representation and strict ordinary Text publication.

The batch passes 833 expression/query tests including doctests, 309 separately
run minimal-profile expression tests, three selected CLI tests and the tightened
resource boundary control. All-feature/minimal Clippy, warnings-denied rustdoc
and formatting/diff checks pass. The original
[comparison recipe](testing/compare_strconv_options.py) passes 938 independent
Decimal/ASCII-representation/locale/native-field controls, including both CLI
commands and en-us/de-de options. Existing read-only ID getters supply unchanged
Jet 4/ACE 2007/ACE 16-family references; only TableCraft executes SQL. Source/JAR
hashes are checked. Native Access/NLS and complete Variant/coercion qualification
remain open; StrConv remains Partial.
