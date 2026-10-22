# Format text layouts

Format now renders borrowed text layouts in scalar and bound read queries, using
its existing optional arguments and shared resource policies. This extends the
[Format profile](FORMAT.md); Format remains Partial.

The [Microsoft Format reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
defines required/optional character slots, fill direction, casing and one/two
string sections. Access query admission is recorded in the
[built-in tracker](ACCESS_BUILTINS.md). These expression functions do not execute
stored VBA.

| Symbol | Current behavior |
| --- | --- |
| `@` | One UTF-16 unit, or a space for an unfilled slot |
| `&` | One UTF-16 unit, or nothing for an unfilled slot |
| `!` | Fill slots from the left; otherwise short input is aligned to the right |
| `<`, `>` | Lowercase/uppercase the complete source through the explicitly selected ICU language before layout |
| Quoted or backslash-escaped characters | Literal output; directives inside literals have no effect |

A first section containing an unescaped text directive selects this profile.
Other symbols in that text section are literal characters. A second section is
selected for both NULL and empty Text; an explicitly empty second section emits
nothing. More than two sections are rejected. An omitted second section uses the
first. Excess source units follow the completed mask. Source case is retained
unless requested; conflicting upper/lower directives are rejected instead of
inventing precedence. Non-Text, non-NULL input to text layouts is currently
rejected; broader Variant conversion remains unfinished.

```sh
tablecraft query eval "SELECT Format('Ab','@@@@'),Format('Ab','!@@@@')"
# Text values: "  Ab", "Ab  "
tablecraft query run "SELECT Format('straße','>\"label:\"@@@@@@')" --case-locale de
# Text value: "label:STRASSE"
tablecraft query run "SELECT Format(Null,'@@;\"missing\"')"
# Text value: "missing"
```

Combined casing uses the same full-context ICU implementation as LCase/UCase.
Casing can expand the source, and layout counts those resulting UTF-16 units.
Mask literals retain their case. Combined masks with nonempty source and a casing directive require
an explicit case language; empty source needs no casing provider there. The existing
standalone `<`/`>` path retains its required case provider even for empty Text. This is an
explicit portable policy, not a claim of Windows NLS equivalence or native
qualification of casing/layout ordering.

Two bounded passes count mapped units and then place them without an intermediate
String or token allocation. Both passes retain cancellation and work accounting;
final output uses the host's charged fallible Text sink. A surrogate pair spanning
adjacent slots survives, including an empty quoted literal between slots. A
nonempty literal between its halves returns UnrepresentableText. This path does
not expose unpaired UTF-16 as ordinary Text, replace it, or convert it to NULL.
Broader raw-string results remain tracked with [byte strings](BYTE_STRING_FUNCTIONS.md).
The existing 257-unit mask truncation and input/output ceilings remain in force.

## Evidence and remaining work

Kernel controls cover optional/required padding, fill directions, overflow,
literals, NULL/empty section routing, contextual/expanding casing and surrogate
boundaries. A fixed-seed generated test compares ASCII layouts with independent
position assignment. Scalar tests exercise each observed cancellation checkpoint
and byte/work/output ceilings. Query tests cover grouping, aggregates,
correlation and plan reuse with changed language/masks. CLI tests exercise both
commands and empty stdout on errors.

The original [independent runner](testing/compare_format_text.py) passes 1326
layout/casing/refusal/native-field controls. Reviewed Jackcess getters supply
ordinary Short Text and NULL/empty values from the four existing original
ScalarFixture databases; all source hashes remain unchanged. Python models assign
UTF-16 positions and use selected Unicode casing controls. They do not execute
Format in Access or constitute native function evidence. No third-party source
or fixture was copied and no dependency was added.

Native Access error/section/coercion/overflow/casing behavior, conflicting-case
precedence and broader raw-string support remain unqualified. Scientific/scaling
numeric masks, broader date sections and regional/Variant completeness remain
separate Format work.
