# StrConv casing, kana and string encoding

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
| 4 / 8 | Bounded Japanese wide/narrow conversion with explicit LCID 1041 |
| 16 / 32 | Basic fullwidth Katakana/Hiragana conversion with explicit Japanese LCID 1041 |
| 36 | Explicit wide-then-Hiragana composition with Japanese LCID 1041 |
| 64 | Decode raw string/Byte-array bytes through an explicit supported string page |
| 128 | Encode representable Unicode as a lossless string-byte intermediate |
| Case setting plus 64 or 128 | Decode before casing; case before encoding |

Conversion is checked/rounded to Integer; an explicit LCID is checked/rounded to
Long and structurally validated without dropping alternate-sort bits. Both slots
now accept raw legacy DATE and execution-local numeric Text conversion. The
[MS-VBAL declaration](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/17afbc5b-e1c7-4717-bfe3-9ae938ef0736)
describes the flag argument and Long LocaleID; the bounded coercion targets here
retain their preexisting profile rather than claiming complete native enum/Variant
call semantics. Unknown flag bits, negative flags and mutually exclusive settings
fail. Standalone width and kana requests require exact LCID 1041. The explicit
36 profile below combines widening and Hiragana conversion. Other combined
width/kana, case or encoding settings remain unsupported. The bounded profiles
below do not claim all East Asian locale or native NLS behavior.

Arguments evaluate eagerly in written order, including the optional LCID, before
flag/locale validation. NULL strings propagate after that validation. Ordinary
scalar sources now use the explicit CStr service before representation conversion;
unsupported CStr carriers still fail. Binary is accepted specifically as a Byte-array
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
| 1041 | ja-JP | 932 (WHATWG Shift_JIS string codec) |
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
system/custom aliases and other multibyte profiles fail without fallback.
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

Encoding uses exact Unicode round trips through the selected string page.
Unmappable characters and unmapped bytes fail; no best-fit mapping, replacement
character or silent byte loss occurs. Native best-fit/replacement behavior and
full Windows DBCS/Unicode code-page support remain compatibility work. Character-page
WHATWG/Windows differences stay explicit in the owning mapping document.

Parser-free hosts can use `Conversion`, `LocaleProfile`, borrowed casing recipes,
`encode_ansi` and `decode_ansi`. TextOutput/ByteOutput sinks own bounded, fallible
allocation and work/cancellation checks; discard prefixes on any failure. The
SQL-enabled evaluator's owning `apply_strconv` returns a scalar or byte operand.
The borrowed text::apply*/TextResult API explicitly rejects this owning operation.

The complete `StringCodePage` API also supports Japanese 932 through metered
`encode`/`decode` methods, and `StringLocaleProfile` supplies its language and
string encoding. Existing `CharacterCodePage`, `LocaleProfile`, `encode_ansi` and
`decode_ansi` retain their single-byte contracts. Their constructors still refuse
Japanese pages/locales; an accepted legacy recipe never silently omits a
multibyte operation. See [Japanese string encoding](JAPANESE_STRING_ENCODING.md)
for exact mappings, duplicate spellings and refusal boundaries.

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
full native error/coercion/NLS behavior, broader width/kana modes, more LCIDs, DBCS and
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
by source NULL propagation and representation checks. Other combined width/kana
flags beyond the explicit 36 profile and unknown LCIDs are never silently replaced
by a supported setting. All written
argument expressions remain eager, so a late expression failure remains visible
before option conversion. NULL options retain InvalidNullConversion. This option-coercion change itself added no LCID, width/kana, DBCS, source
carrier or default-locale implementation; standalone kana is described below.

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


## Scalar string sources

StrConv now shares the [scalar byte-source conversion](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources)
profile. A numeric source uses the host's explicit NumberFormat before casing or
ANSI conversion; the call's LCID continues to select casing/code-page behavior
and does not invent a number/date formatting policy. Legacy date sources use the
explicit DateTextFormat. Binary arrays retain their existing conversion-64-only
route; raw string intermediates never round-trip through CStr.

Source coercion follows existing flag/LCID validation and NULL propagation.
`StrConv(1234,0)` produces Text `1234` under an explicit numeric profile. Casing
and encoding then use the same metered paths as an explicit Text source. Other
combined width/kana modes, strict Unicode publication and full native/Variant qualification
remain open. The shared scalar-source runner exercises these calls alongside the
byte slices/search/observations without executing database content.

## Ordered expression-state admission

StrConv now retains eager expression states through flag conversion/validation,
explicit LCID conversion/lookup and source admission. Error source data no longer
fails before those controls: `StrConv(CVErr(11),-1)` reports Domain, and a supplied
LCID zero fails structural validation before source Error admission. Flag and
LCID Error values give TypeMismatch; Missing gives NonScalarExpression when
reached. Source Error requires explicit CStr or numeric conversion, rather than
implicitly acquiring its code or English spelling. NULL propagation still occurs
only after valid controls. These are preserved TableCraft validation rules;
native Access error precedence remains unqualified.

Host Empty has explicit targets: empty Text for the source and numeric zero for
flags/LCID. Zero flags retain the original representation without casing or
encoding. An Empty LCID still selects the existing invalid zero identity; it
does not discover a system locale. Empty non-NULL sources still require the
appropriate casing/code-page policy when that operation needs one. SQL omitted
slots keep their existing normalization; no Empty constructor is added.

The working `ExpressionByteOperand` input owns either scalar expression data or
an already validated raw string. It moves buffers without cloning and cannot
make Error/Empty/Missing stored values or published byte-string results. The
source getter runs after controls; the original materialized API shares the same
preparation/rendering service. Standalone and bound query evaluation preserve
states through CVar and preserve nested byte producers without Unicode
publication. A zero-flag raw input retains its original buffer, including odd or
unpaired UTF-16 bytes; ordinary Text publication remains strict. Other byte
functions retain their own state/admission boundaries.

Written expressions still evaluate once, in order. Operational failures are never
stored as Error data: `StrConv(CVErr(11),0,1/0)` fails division immediately, and
`StrConv(ChrB(300),CVErr(11))` fails its source operation before flag admission.
Per-Text, cumulative byte/work, cancellation, code-page and output guards remain;
no new locale, flag, runtime, return category or dependency is added.

Microsoft's [Access StrConv contract](https://support.microsoft.com/en-us/access/functions/strconv-function),
[Empty target table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e2637723-3010-449d-bead-9f17533dc105)
and [explicit Error conversion distinction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/cverr-function)
guide the existing query-function profile. They do not qualify Windows NLS,
LCID-zero default handling or native error-order behavior.

Three expression tests cover state/control order, host Empty, zero-byte-budget
raw buffer identity, output refusal, every observed successful cancellation
checkpoint, CVar and once-only Rnd controls. Query/CLI tests add derived Error
values, explicit conversion, unpaired raw intermediates and buffered failures.
The original [state runner](testing/compare_strconv_states.py) passes 154 controls
(114 scalar and 40 native sequential/indexed) across four unchanged registered
inputs. The existing mapping/casing runner also passes 12,046 scalar controls;
its optional native branch was not rerun. Only TableCraft executes SQL. Native
Access/Windows, full Variant and remaining byte/Format semantics stay open.


## Explicit Japanese kana conversion

Standalone conversion 16 maps basic fullwidth Hiragana to Katakana; 32 maps basic
fullwidth Katakana to Hiragana. Both require the explicit, exact Japanese LCID
1041. No host locale is inferred, and 1041 does not introduce Japanese casing or
an ANSI/DBCS page into the preceding locale table. Missing/non-Japanese LCIDs and
alternate-sort aliases fail. The explicit wide/Hiragana profile below is the only
combined width/kana mode; other combined case, width and encoding flags remain refused.

```sh
tablecraft query eval "SELECT StrConv('ひらがな',16,1041) AS Label"
tablecraft query run "SELECT StrConv('カタカナ',32,1041) AS Label"
```

The results are `ヒラガナ` and `かたかな`. The Access
[StrConv query contract](https://support.microsoft.com/en-us/access/functions/strconv-function)
defines these Japanese-only flags. Unicode documents the corresponding scripts
and voiced marks in [chapter 18](https://www.unicode.org/versions/Unicode18.0.0/core-spec/chapter-18/).
The implementation maps corresponding basic letters and iteration marks, including
small KA/KE and VU. Katakana voiced WA/WI/WE/WO expand to the corresponding
Hiragana base plus combining voiced mark, since those precomposed Hiragana letters
do not exist. Existing combining and spacing marks retain their code points;
there is no whole-string normalization. Halfwidth Katakana remains unchanged:
Microsoft's [mapping guidance](https://learn.microsoft.com/en-us/windows/win32/intl/handling-sorting-in-your-applications)
distinguishes script conversion from the separate fullwidth flag.

Unrelated text remains unchanged. Source digraphs without a qualified counterpart,
Katakana phonetic extensions when converting to Hiragana, and historical or
supplementary kana fail explicitly. This is a bounded TableCraft conversion
profile, not a claim of full Windows NLS or native Access equivalence.

All written arguments remain eager. Flags and LCID are validated before source
NULL propagation or strict publication of retained String bytes. Odd bytes and
unpaired UTF-16 fail; byte slices inspect only their active range. Scalar sources
retain the existing explicit CStr policies. Conversion shares work, cancellation,
allocation and per-Text limits; voiced-mark expansion can exceed the output limit
although the input fits. A reused plan does not retain a prior Japanese selection.

Parser-free hosts retain the original public `Conversion::from_code` contract:
it still refuses 16/32, so existing casing/encoding component recipes cannot
silently omit a newly accepted transformation. The evaluator uses a private,
exhaustive operation dispatch. The separate `KanaConversion` streaming writer
exposes the bounded mapping itself; its caller owns locale validation and output
limits.

The focused batch passes 17 expression tests and six bound-query tests, including
existing StrConv controls, explicit public-parser refusals, natural Text/NULL
results, aggregate/correlated visitors, raw intermediates, plan reuse, exact locale
selection, eager failures, cancellation and expansion limits.

The original [kana comparison runner](testing/compare_strconv_kana.py) passes
3,024 success controls and 32 refusals through both CLI modes (1,040 commands).
Python Unicode 16.0 character-name counterpart lookup supplies the independent
answers; canonical decomposition is used only when a voiced source lacks a
precomposed counterpart. It does not import production mappings or normalize
whole strings. Single letters, iteration marks, deterministic mixed strings,
voiced expansions, halfwidth/combining preservation, NUL and retained byte slices
are covered. The [retained report](testing/results/strconv-kana-2026-10-24.json)
records runner, source and unchanged executable hashes. Native Access/NLS execution
remains deferred; StrConv remains **Partial**.


## Explicit Japanese width conversion

Standalone 4 widens qualified Japanese halfwidth characters and ASCII; 8 narrows
qualified fullwidth counterparts. Both require exact explicit LCID 1041, following
the same option/state/raw publication boundaries as script conversion. They do not
supply Japanese casing or a DBCS page. Combined width/script/case/encoding requests
remain refused; valid NULL sources propagate after control validation.

```sh
tablecraft query eval "SELECT StrConv('ABC 123 ｶﾞﾊﾟ',4,1041) AS Label"
tablecraft query run "SELECT StrConv('ＡＢＣ　１２３　ガパ',8,1041) AS Label"
```

The outputs are `ＡＢＣ　１２３　ガパ` and `ABC 123 ｶﾞﾊﾟ`.
The [Access contract](https://support.microsoft.com/en-us/access/functions/strconv-function)
documents width settings 4/8 for East Asian locales. This implementation admits
only the explicit Japanese selector; other East Asian locale profiles remain open.
Unicode [width decomposition mappings](https://www.unicode.org/Public/UCD/latest/charts/nameslist/ff00/)
supply ASCII/fullwidth and Japanese punctuation/Katakana counterpart identities.
Space maps to/from U+3000; white-parenthesis forms U+2985/U+2986 map to/from
U+FF5F/U+FF60. No UTF-8 byte width or stored byte-array conversion is
implied by the historical single-byte/double-byte terminology.

Widening composes adjacent qualified Hiragana/Katakana bases with voiced or
semi-voiced marks, including halfwidth or spacing marks. Microsoft's
[LCMapStringEx contract](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-lcmapstringex)
explicitly describes this behavior for already-fullwidth kana. Narrowing decomposes
precomposed voiced Katakana when both resulting parts have halfwidth counterparts;
for example `ガ` becomes `ｶﾞ`. Hiragana keeps its script. Standalone halfwidth marks
widen to combining U+3099/U+309A, and their combining counterparts narrow to halfwidth marks. Spacing marks are
preserved unless a qualified widening pair actually composes; their standalone
compatibility decomposition is not used. Consequently narrowing `カ゛` yields
`ｶ゛`, while widening it composes `ガ`; this profile does not claim inverse
representation for spacing-mark sequences. General combining sequences are not reordered or normalized.

The selected `unicode-normalization` library supplies canonical character
composition/decomposition only inside qualified kana branches. There is no
whole-string NFC/NFKC: circled digits, ligatures, square unit symbols and unrelated
compatibility characters retain their representation. Output is streamed with a
single pending character for widening and a bounded two-character decomposition
for narrowing; the library's iterator/string normalization APIs are not used.

Fullwidth small KA/KE and WI/WE, or voiced WI/WE without halfwidth bases, fail when
narrowing. Halfwidth Hangul and the fullwidth currency/halfwidth symbol block remain
explicitly unsupported in both directions. These known source forms are refused; other characters outside the qualified
width mappings retain their representation. Further native mappings, locale
coverage and other combined flags remain compatibility work.

The original public `Conversion` parser still refuses width flags, preserving its
component-recipe contract. The evaluator's private exhaustive dispatch selects the
separate `WidthConversion` writer. Raw retained Strings strictly publish only their
active bytes; malformed selected UTF-16 fails. Arguments remain eager, controls
precede source admission, and numeric/date sources retain explicit CStr policies.
Every source character and output append shares cancellation/work/allocation
controls. ASCII widening and voiced narrowing can exceed the per-Text output limit
even when the source fits. No partial buffered query output is published on failure.


Twenty focused expression/kernel tests and eight bound-query controls pass,
including the five new width tests. They cover field/correlated/aggregate use,
plan reuse after failed locale/flag selections, public-parser refusals, eager
errors, NULL/raw publication, discarded invalid slice prefixes, per-Text expansion
and cancellation at every observed scalar checkpoint. Strict SQL/minimal-feature
expression and selected query Clippy checks pass. Native Access/NLS width results
remain unqualified; StrConv remains **Partial**.


The original [width comparison runner](testing/compare_strconv_width.py) passes
5,544 success controls plus 30 refusals across 1,878 CLI commands in eval/run.
Its independent Python Unicode 16.0 model derives 160 width pairs from character
properties and uses local kana composition/decomposition only. Single mappings,
voiced sequences, spacing-mark asymmetry, deterministic mixed strings, unrelated
compatibility characters, raw slices, NUL and control/error refusals are covered.
The [retained report](testing/results/strconv-width-2026-10-24.json) records unchanged
binary, source and manifest hashes. An [excluded initial attempt](testing/results/strconv-width-precorrection-2026-10-24.txt)
found missing white-parenthesis counterparts; those two pairs were implemented and
pinned before the final replay. The failed attempts are not passing evidence or
included in completed control totals. The earlier kana runner now uses combined
flag 5 for its unsupported-width-control slot, since standalone 4 is implemented;
the older kana report remains a historical result for its recorded runner hash.


## Explicit wide-then-Hiragana composition

Conversion 36 (`4 | 32`) now applies the existing widening kernel before the
existing Hiragana kernel, requiring the exact explicit Japanese LCID 1041. This
is a TableCraft engineering composition informed by Microsoft's documented
mapping behavior; it is not a native Access StrConv ordering qualification.

The [MS-VBAL StrConv contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/17afbc5b-e1c7-4717-bfe3-9ae938ef0736)
allows nonexclusive flags to be combined but does not establish a universal
operation order. Microsoft's [mapping guidance](https://learn.microsoft.com/en-us/windows/win32/intl/handling-sorting-in-your-applications)
specifically requires FULLWIDTH together with HIRAGANA to map halfwidth Katakana
to Hiragana. Its [LCMapStringEx contract](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-lcmapstringex)
also documents fullwidth voiced composition, such as `は゛` becoming `ば`.
Those specific behaviors motivate widening before script conversion here.
They do not establish ordering for other width, script, case or encoding mixes.

```sh
tablecraft query eval "SELECT StrConv('ｶﾞﾊﾟｳﾞ ABC',36,1041)"
```

The result is `がぱゔ　ＡＢＣ`. The existing width and kana coverage/refusals apply
at each stage. Halfwidth voiced sequences first become composed Katakana, then
Hiragana; fullwidth voiced WA/WO retain the kana kernel's explicit combining-mark
representation. Characters outside the qualified mappings keep their existing
representation. No general normalization or new mapping table is introduced.

Controls validate before source admission, and written expressions remain eager.
Valid controls allow NULL propagation; missing/non-Japanese/alternate-sort LCIDs,
malformed raw Unicode, contradictory flags and other unsupported combinations
retain their existing errors. Input evaluation and coercion occur once. Both
transformation stages use the same cumulative allocation/work/cancellation pool,
with each intermediate and final Text subject to the existing per-Text ceiling.
No partial result is returned if either stage fails.

The legacy public `Conversion` component recipe still refuses 36, so existing
callers cannot silently omit a newly accepted transformation. The evaluator's
private exhaustive operation dispatch owns the complete two-stage operation.
All other previously unsupported combined width/kana/case/encoding values remain
unsupported, including 20, 37, 100 and 164. Native NLS/Access equivalence and wider
combined modes remain separate compatibility work.

Three focused scalar evaluator tests and one bound-query test cover the new
composition. The accepted scoped checks pass 28 expression tests and 19 query
tests, with strict Clippy, minimal-feature compilation and scoped formatting.
The [verification record](testing/results/read-query-wide-hiragana-2026-10-25.json)
retains exact commands/log hashes, the four new controls and 43 existing
regressions. CLI and native execution evidence are separate.

The scoped wide/Hiragana checkpoint passes 51 distinct expression/query/CLI tests,
including five new controls, strict linting, minimal expression features and
formatting. See [verification](testing/results/read-query-wide-hiragana-2026-10-25.json).
