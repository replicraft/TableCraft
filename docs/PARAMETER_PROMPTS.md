# Literal parameter prompts

Typed query parameters now support bracketed prompt text containing punctuation,
including `Enter No. of items`, `P.Q`, `Floor!` and `Floor>= minimum`. Brackets
enclose one name: `[P.Q]` is distinct from the qualified field `P.Q` and does not
become a path or executable expression. The parser retains the spelling in the
existing owned Name/FieldReference model; binding determines whether an unqualified
reference denotes an input field or a declared parameter.

[Microsoft's PARAMETERS reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql)
documents bracketed names containing spaces or punctuation. Its
[parameter-query guide](https://support.microsoft.com/en-US/Access/use-parameters-to-ask-for-input-when-running-a-query)
describes prompt text in criteria. TableCraft retains explicit typed declarations
and supplied values; it does not display an input dialog or infer a declaration
from an unresolved name.

## Parser and binding

Bracketed names in parameter declarations and unqualified expressions may contain
dot/bang/grave-accent punctuation. Empty names, controls and embedded bracket
delimiters still fail. Object declarations, table names, aliases, bang-member paths and callable
identifiers retain their existing rules. Generated heading fields now also allow
[qualified literal dot references](NAMED_CROSSTABS.md), such as `c.[Sales.Q1]`. Names with punctuation
excluded from object names cannot qualify a member or wildcard. Native object-name validation has not
been relaxed into a general dotted-name or escaped-bracket grammar.

The binder keeps field-before-parameter resolution, exact declared carriers,
missing/duplicate-value failures, reusable prepared plans and isolated named-query
scopes. A literal prompt can be used inside a supported nested read; its text does
not execute a function even if it resembles `NeverRunMe()`. Raw `Forms!F!C` remains
an application member reference whose execution is deferred; `[Forms!F!C]` is one
literal name. Parsing the former does not supply a GUI, COM or VBA binding.

## Native saved declarations

The Access adapter accepts punctuated and Unicode literal prompt names. Accented,
Cyrillic, CJK and supplementary-character spellings retain their decoded text; the
adapter no longer imposes an ASCII-only filter absent from the owned parser. One
complete stored bracket pair is removed once while original metadata stays unchanged. Dots in unbracketed stored prompt names are
literal, following the reviewed Jackcess 5.0.4
[QueryImpl.toParameterName](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
renderer. A raw name containing bang separators with nonempty parts and no boundary
spaces is a qualified control reference and remains unsupported. Thus raw
`Ask!Now` is refused, while stored `[Ask!Now]`, `Floor!` and `Ask! now` are literal
prompt spellings under that reference profile. Mixed/nested bracket paths and
malformed/control characters still fail explicitly.

The adapter uses bounded declaration parsing to obtain a validated owned name,
retaining current type/length checks, ASCII-insensitive duplicate rejection,
cumulative work/allocation/syntax limits and publication cancellation. No new
dependency, native write path or unchecked constructor was introduced. Matching
uses the existing shared policy: ASCII case-insensitive, otherwise exact.
Consequently `éP` and `ép` are duplicates; `é`, `É` and `e` plus a combining acute
remain distinct spellings. This is not native Unicode/NLS identity qualification.
No normalization, Unicode case folding, machine locale or comparer is inferred.
Existing UTF-8 byte-based name/syntax ceilings remain in force, including for
multibyte names; the full Access naming/regional/error-order scope remains open.

## CLI assignments

`--param NAME=TYPE:VALUE` remains available. A bracketed assignment name removes
ambiguity when the name itself contains `=`:

```sh
tablecraft query run \
  'PARAMETERS [Enter No. of items] LONG; SELECT [Enter No. of items] AS Total' \
  --param '[Enter No. of items]=long:3'

tablecraft query run \
  'PARAMETERS [Floor>= minimum] LONG; SELECT [Floor>= minimum] AS Total' \
  --param '[Floor>= minimum]=long:3'
```

The same assignment syntax applies to Unicode stored prompts, for example
`--param '[最低値]=long:2'` when a selected saved query declares that exact name.

Shell quoting is still needed for spaces/metacharacters. Text values keep all
content after their type separator: `P=text:a=b:c` and `[P=Q]=text:a=b:c` preserve
`a=b:c`. Malformed bracket assignments, normalized duplicate names, invalid
carriers and existing name/count/argument ceilings remain errors. Supplied values
are never interpreted as SQL. Raw inspection and `query saved` never request
parameter values; explicit execution without required values fails with buffered
stdout. Native `--password` prompting is a separate feature.

## Evidence and remaining scope

Four parser tests include 128 generated prompts and unchanged object/member/call
guards. Four query tests check dotted-field distinction, typed plan reuse, field
precedence, nested literals and compatible/isolated named scopes. Six adapter
tests cover raw/quoted names, Unicode identity, four native header layouts,
malformed/control refusals, duplicates, generated names and all measured
cancellation checkpoints. Four CLI tests cover inline/native saved/named paths,
assignment delimiters, text preservation, missing values and buffered failures.

The original [SavedPromptFixture.java](testing/SavedPromptFixture.java) now adds
twenty-three inert prompt definitions to our fourteen-query baseline. Four fresh Jet 4 / ACE
2007/2016/2019 files pass 464 independent getter/rendering/direct/saved/named/refusal
controls through [compare_saved_prompts.py](testing/compare_saved_prompts.py).
They include raw/quoted Unicode prompts, composed/decomposed spellings,
derived reads, UNION, crosstabs and a deferred control reference.
The reference renders metadata, not results; explicit expected rows control
execution. [Fixture records](FIXTURES.md#literal-parameter-prompt-inputs) retain
provenance/hashes; source bytes remain unchanged. Windows Microsoft Access prompt,
collation, execution/error-order and save/reopen qualification remain unrun.
