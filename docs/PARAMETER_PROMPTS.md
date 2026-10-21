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
delimiters still fail. Object declarations, table names, aliases, qualified member
names and callable identifiers retain their existing rules. Names with punctuation
excluded from object names cannot qualify a member or wildcard. Native object-name validation has not
been relaxed into a general dotted-name or escaped-bracket grammar.

The binder keeps field-before-parameter resolution, exact declared carriers,
missing/duplicate-value failures, reusable prepared plans and isolated named-query
scopes. A literal prompt can be used inside a supported nested read; its text does
not execute a function even if it resembles `NeverRunMe()`. Raw `Forms!F!C` remains
an application member reference whose execution is deferred; `[Forms!F!C]` is one
literal name. Parsing the former does not supply a GUI, COM or VBA binding.

## Native saved declarations

The Access adapter now accepts punctuated literal names in the existing ASCII
native parameter profile. One complete stored bracket pair is removed once while
original metadata stays unchanged. Dots in unbracketed stored prompt names are
literal, following the reviewed Jackcess 5.0.4
[QueryImpl.toParameterName](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
renderer. A raw name containing bang separators with nonempty parts and no boundary
spaces is a qualified control reference and remains unsupported. Thus raw
`Ask!Now` is refused, while stored `[Ask!Now]`, `Floor!` and `Ask! now` are literal
prompt spellings under that reference profile. Mixed/nested bracket paths and
malformed/control characters still fail explicitly.

The adapter uses bounded declaration parsing to obtain a validated owned name,
retaining current type/length checks, normalized ASCII duplicate rejection,
cumulative work/allocation/syntax limits and publication cancellation. No new
dependency, native write path or unchecked constructor was introduced. Unicode
native parameter identity and full Access regional/error-order behavior remain
unqualified. Existing wider SQL-only spelling support is not native qualification.

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
precedence, nested literals and compatible/isolated named scopes. Five adapter
tests cover raw/quoted names, four native header layouts, malformed/control/Unicode
refusals, duplicates, generated names and all measured cancellation checkpoints.
Three CLI tests cover inline/native saved/named paths, assignment delimiters,
text preservation, missing values and buffered failures.

The original [SavedPromptFixture.java](testing/SavedPromptFixture.java) adds thirteen
inert prompt definitions to our fourteen-query baseline. Four fresh Jet 4 / ACE
2007/2016/2019 files pass 264 independent getter/rendering/direct/saved/named/refusal
controls through [compare_saved_prompts.py](testing/compare_saved_prompts.py).
They include derived reads, UNION, crosstabs and a deferred control reference.
The reference renders metadata, not results; explicit expected rows control
execution. [Fixture records](FIXTURES.md#literal-parameter-prompt-inputs) retain
provenance/hashes; source bytes remain unchanged. Windows Microsoft Access prompt,
collation, execution/error-order and save/reopen qualification remain unrun.
