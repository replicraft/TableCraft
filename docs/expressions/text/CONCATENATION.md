# Contextual string concatenation

Access `&` now concatenates supported scalar operands through the shared evaluator.
Text is retained exactly; Boolean uses `True`/`False`; numeric and legacy/extended
date operands reuse [CStr](CSTR.md). Numeric output requires an explicit
`NumberFormat`, including integer operands. Dates require `DateTextFormat`.
No ambient locale or CLI cell renderer supplies expression text.

```sh
tablecraft query eval "SELECT 'ID=' & 9007199254740993" --number-format invariant
tablecraft query eval "SELECT #2024-01-02# & '!'" --date-format iso
tablecraft query run "SELECT 'ID=' & ID FROM Contacts" \
  --database example.accdb --number-format invariant
```

One NULL operand contributes empty text; two NULLs return NULL. Host-supplied
Empty expression data now contributes empty Text; Missing remains refused.
Query Nz already returns empty Text when its fallback is omitted, and retains
that behavior. The separate `+` path still concatenates Text/Text, propagates a participating
NULL and uses its existing contextual numeric rules for mixed numeric operands.
Both operands evaluate in written order before scalar formatting: a later division
by zero is not hidden by NULL or a prior operand needing an output policy.
Unselected conditional branches remain eager.

Binary and GUID implicit formatting remain unsupported; callers use explicit
qualified conversions where available. Error operands now fail TypeMismatch after
both operands evaluate. They never render as CLI `#ERROR(...)` text inside an
expression; explicit CStr remains distinct. Missing, odd-byte string and array
concatenation remain outside this extension. See the
[expression consumer contract](../../query/aggregates/EXPRESSION_AGGREGATES.md#concatenation-and-numeric-admission). Full native
Variant, byte-array and regional behavior remain compatibility gaps.

## Lossless UTF-16 concatenation

Ampersand now retains complete raw UTF-16 units through its operands and result.
For example, `ChrW(55357)&ChrW(56832)` and
`Left('😀',1)&Right('😀',1)` publish `😀` without first decoding either
surrogate half. Nested Len/LenB, AscW/AscB, CVar, type inspection, byte producers
and ordinary slices can consume the intermediate. `Len(ChrW(55296)&NULL)` returns
one, while direct publication of `ChrW(55296)&NULL` remains refused. Opaque Binary
is not interpreted as Text, and raw strings do not become stored/parameter cells.

Both operands evaluate eagerly before state admission and scalar formatting.
Error remains TypeMismatch even beside NULL or a raw string; operational errors
in a later operand remain visible. Raw operands must contain whole UTF-16 units;
odd-byte concatenation is explicitly unqualified and refused, including pairs
whose combined byte count would be even. This does not extend the overloaded `+`
operator or arbitrary raw-string comparison/selection semantics.

Scalar-only pairs retain the existing borrowed UTF-8 join. When a raw operand is
present, ordinary scalar conversion still uses explicit CStr policies, and Text
encodes through the existing metered UTF-16 path. Raw concatenation checks Long
byte-length overflow and charges the complete output before fallible reservation.
Copying checks cancellation/work at bounded chunks. A raw string joined with NULL
can transfer its existing buffer without a new payload allocation. Strict final
publication enforces the selected Text ceiling. Bound execution retains both
runtime Text policies and declaration provenance through these intermediates.

The [MS-VBAL ampersand contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9f072ffc-e943-4fcc-a4d0-f3c7db96abd9)
defines String joining, Empty/NULL admission and Error refusal; Access query
availability is established by the query examples cited below. These sources
motivate the explicit complete-unit profile, not native qualification of
surrogate handling, odd-byte strings or broader Variant/Byte-array coercion.

The original ampersand expansion passed 1,542 source-free controls over both
query commands. Python constructs UTF-16LE bytes independently, observes units,
decodes only final valid output and checks refused invalid publication. Seeded
unit pairs include zero/control characters, paired/unpaired surrogates, NULL,
Empty, policies and eager failures. It also checks the CLI's existing escaped
display-cell contract rather than treating that output as raw JSON. Native
fixtures, Windows Access and benchmarks were not run for that expansion. The
subsequent plus expansion below records the recipe's current combined coverage.

## Lossless String addition

The String/String branch of `+` now retains complete raw UTF-16 units just as
ampersand does. `ChrW(55357)+ChrW(56832)` and
`Left('😀',1)+Right('😀',1)` publish `😀`. Nested byte/unit observations,
inspection, CStr/CVar and slices can consume the intermediate. Direct invalid
final output remains refused; qualified [query cells](RETAINED_STRINGS.md) now
retain the intermediate. Odd-byte concatenation is
still unqualified and refused when a join is required.

Plus keeps its separate NULL rule: `ChrW(55296)+NULL` returns NULL after both
operands evaluate. This needs no Unicode decoding, string join or numeric policy;
the same NULL shortcut applies to an already admitted odd-byte String. An eager
failure such as `(ChrW(55296)+NULL)+(1/0)` still aborts. Error/Missing remain
refused. [Host Empty arithmetic](../numeric/EMPTY_ARITHMETIC.md) now retains an existing
String under plus without parsing or copying; other numeric operators use their
reviewed effective targets. Query Nz's existing Text behavior is unchanged.

String/numeric pairs still perform numeric addition. A raw string must first
publish representable Text, then use the existing explicit NumberFormat and
numeric target rules. `LeftB('12',4)+1` yields numeric thirteen with an explicit
number policy, whereas `LeftB('12',4)+'1'` yields Text `121`. Binary/GUID/extended-
date and non-finite counterpart admission retains its arithmetic refusal rather
than silently concatenating or producing NULL.

The shared evaluator's `PreparedAddition` returns either a completed raw
String/NULL result or materialized operands for existing scalar/declared dispatch.
Query execution retains each operand's text policy and original Scalar/Variant
category through that handoff. The explicit declared profile additionally uses
those categories to select the mixed String/numeric operation below; the default
materialized profile retains numeric addition. No declaration is inferred from
decoded values or a successful conversion.

Scalar-only pairs retain existing dispatch and borrowed Text joins. Raw String
joins reuse the checked, cumulatively charged ampersand buffer-copy kernel;
numeric publication and normalization retain their own charges. All existing
work/cancellation, output ceilings and buffered CLI failure rules remain active.

Microsoft's [MS-VBAL plus contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/91621d4b-3da3-4fe9-9581-adda857efe05)
distinguishes String joining, numeric addition and NULL results. The Access text
expression references below establish query use. The expanded independent
concatenation recipe passes 2,918 source-free controls over both operators;
Python models UTF-16LE joining and final decoding independently. Existing byte
and numeric Text recipes still pass 35,336 and 660 controls. Native Access,
fixture replays and broader Variant/odd-byte qualification remain open.

## Shared execution and metadata

Source-free evaluation, bound queries, named/derived consumers, grouping, nested
subqueries and native sequential/indexed scalar sources share the same evaluator.
Prepared plans retain no output locale; each execution uses its own number/date
policies. No database bytes, stored schema or parameter carrier changes.

For known supported scalar operands, `&` output metadata is unconstrained Text.
NULL is allowed only when both operands can be NULL; two null-only operands
establish null-only output. Unknown/unsupported families stay Dynamic. Inference
never formats values, reads rows or activates a provider. Operand Text constraints
are not reused as concatenated output length limits.

Metadata-only named crosstab binding still permits only its qualified Text/NULL
constant concatenations. A heading needing scalar formatting is Unsupported there,
so explicit execution-stage discovery resolves it under the current context and
shared budgets. Missing execution policies remain visible errors; binding does
not infer settings or scan data to invent headings.

## Resource and evidence limits

Already materialized Text/NULL operands are borrowed during the join. Scalar-to-
Text conversions charge their emitted intermediate bytes through CStr; the final
joined Text is charged separately before fallible allocation. Shared cancellation,
work and the selected per-value Text ceiling (64 KiB default, up to 1 MiB by
explicit execution option) remain active. Failed work is
not refunded. Queries and ordinary CLI output remain buffered, with no success
stdout on a later conversion, quota or formatting failure.

Focused tests cover exact LongLong/fixed-point digits, date and numeric profiles,
Boolean/NULL behavior, policy reuse, metadata-only sources, derived/grouped/nested
queries, named crosstab discovery, all observed conversion cancellation checkpoints,
intermediate/final allocation caps and unchanged input values. The original
[Python runner](../../testing/compare_concatenation.py) passes 190 controls through eval
and run, including unchanged native inputs and both directions of indexed seeks.
Python integer/Decimal/calendar models and read-only Jackcess getters provide
expectations. Exact Decimal tests use lexical CDec input to isolate them from the
separate floating SQL literal policy. [Provenance](../../testing/FIXTURES.md#original-contextual-concatenation-controls)
records the existing input sets. No benchmark or speed claim is introduced.
Native Windows Access SQL, formatting, coercion and error-order comparisons remain
unrun; these controls establish the explicit implementation profile.

Microsoft's [Access expression examples](https://support.microsoft.com/en-us/access/examples-of-expressions)
establish `&` use in calculated query fields. The
[operator reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ampersand-operator)
and [MS-VBAL operator contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9f072ffc-e943-4fcc-a4d0-f3c7db96abd9)
describe string coercion and NULL handling. They are implementation references,
not native qualification of our explicit rendering policies or deferred operands.

## Declared mixed String and numeric addition

The existing explicit `--declared-arithmetic` profile now selects mixed `+` from
the original declarations before Unicode publication or numeric conversion.
Default materialized execution and source-free `query eval` retain their previous
mixed numeric-addition behavior. No new profile flag or function is added.

| String operand declaration | Numeric operand declaration | Explicit declared result |
| --- | --- | --- |
| Scalar | Scalar | TypeMismatch |
| Scalar | Variant | String concatenation through existing CStr policies |
| Variant | Scalar, Variant or unknown | Numeric addition through existing conversion/promotion |
| Scalar | Unknown | ArithmeticDeclaration |
| Unknown | Scalar, Variant or unknown | ArithmeticDeclaration |

The same table applies in either written order. For example, `'2'+CInt(3)` fails
TypeMismatch, `'2'+CVar(CInt(3))` returns Text `23`, and
`CVar('2')+CInt(3)` returns Double 5 under the declared profile with the explicit
en-US number policy. Reversing the concatenating operands returns `32`.
A known Variant numeric operand cannot resolve an unknown String declaration:
that String could select concatenation or addition. No record sample or
successful parsing invents a category.

String/String still concatenates independently of categories. Numeric/numeric
still enters the existing selected-width/overflow logic. NULL, host Empty and
Error/Missing retain their existing plus admission; operand expressions execute
eagerly before any branch decision. Unsupported/nonnumeric and nonfinite scalar
carriers remain refused. The query planner conservatively describes successful
mixed Text/numeric plus output as Dynamic because execution options can now change
its carrier; prepare remains metadata-only.

The String branch retains raw complete UTF-16 units, including an unpaired
surrogate that a later unit consumer can inspect. It formats the numeric operand
through existing CStr, preserving exact LongLong, Currency and Decimal output and
requiring the existing numeric/date output policies. It never parses a nonnumeric
String prefix merely to concatenate it. Odd-byte joins remain refused. The
numeric branch strictly publishes reached raw Strings and reuses the existing
NumberFormat and numeric target rules. Scalar/Scalar mismatch or an unresolved
category does not first decode a malformed String. Entry work, formatting,
copying, final Text ceilings and cancellation remain cumulatively charged.

Microsoft's [plus operator reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
separately specifies non-Variant versus Variant String/numeric selection. The
[MS-VBAL plus contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/91621d4b-3da3-4fe9-9581-adda857efe05)
provides the runtime String/String branch and general arithmetic linkage. These
motivate this additional explicit declaration policy. They do not establish how
native ACE SQL classifies every expression, parameter, field or function; no
Windows comparison qualified the new branch selection.

Five direct API tests cover the complete nine-category mixed matrix in both
orders over materialized/active-range Strings, exact carrier/formatting controls,
String/String and expression states, malformed branch-specific admission,
unknown categories, final Text ceilings and every observed cancellation point.
Prepared-query composition and metadata controls live in `declared_addition`.

## Constructed String policy provenance

Concatenation uses field policy only from operands that contribute an actual
String or a conservatively retained NULL policy. A numeric, Boolean or date
result does not contribute latent Text policy from an unselected conditional
branch when the existing conversion service formats that result. For example,
with different LegacyText and ModernText field policies,
`LegacyText & IIf(True,2,ModernText)` retains LegacyText's policy. The explicit
declared profile applies the same rule when `+` selects String construction.
Numeric `+` retains its separate arithmetic dispatch and failure rules.

Constructed results retain their runtime policy through named/derived reads,
subqueries, aggregation, grouping and set operations. Conservative public result
metadata stays available without scanning rows; it cannot overwrite a completed
runtime construction with a policy belonging only to a converted non-String.
Actual String policies from all contributing rows still merge and conflicting
identities still refuse. This does not choose precedence between different
native String orders.

Stored Text NULLs and conditional NULL results retain their existing conservative
policy contribution; this change does not establish a distinction between their
origins. A literal NULL has default provenance and retains existing `&`/`+`
nullness behavior. Operand evaluation stays eager, conversion errors retain their
original classification, and raw UTF-16, output and cumulative work limits remain
unchanged. This is a TableCraft provenance correction, not native Access collation
qualification.

UNION resolves each arm before merging output provenance; an empty arm retains
its static policy, including a real declared Text conflict. Crosstabs with
construction-sensitive or derived pivots retain candidates until runtime policy
is known. Policy-sensitive fixed-heading duplicate checks then occur during
explicit execution after observing rows; names, types and NULL headings remain
validated first. Ordinary base-field pivots retain their earlier duplicate
checks before scanning. Binding remains metadata-only. Candidate retention stays
bounded by the existing cumulative allocation and work allowances.
