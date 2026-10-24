# Closed query Eval expressions

`Eval(value)` now runs in scalar and bound read-only queries. Microsoft's
[Eval reference](https://support.microsoft.com/en-au/access/functions/eval-function)
includes direct SQL examples and describes evaluating expression text, including
numeric operations and function calls. This is separate from the CLI command
named `query eval`, and does not supply VBA execution.

## Implemented input and result profile

Text must contain exactly one expression under the existing bounded Access
expression grammar. The strict parser refuses statement terminators, extra
statements, SELECT/UPDATE commands and expression suffixes. Semicolons inside
quoted strings remain data; ordinary trailing comments remain supported. Leading
form-control `=` expressions and syntax outside the parser's profile are not
normalized into another language.

Numeric and Boolean input uses the existing CStr conversion before parsing.
Numeric carriers require an explicit number-format policy, supplied in the CLI
with `--number-format invariant`, for example. There is no machine-locale lookup
or silent decimal-separator fallback. The selected formatting must produce valid
expression syntax; localized decimal-comma conversion is not a separate dynamic
parser dialect. NULL input fails InvalidNullConversion; DateTime, extended date,
Guid and Binary inputs fail TypeMismatch in this initial argument profile.

Closed expressions use implemented literal/operator/function rules, including
nested Eval and explicit Array consumers. `Chr$` now resolves to the same checked
character kernel as Chr, including NULL rejection and code-page requirements;
this is a specific supported spelling, not a general `$` alias rule. Ordinary
unsupported function arities still fail before operand execution. The dynamic
shape validator does not implement missing functions or change their coercion.

Scalar results retain their existing carriers. The expression-result path also
retains explicit CVErr data through nested Eval and its consumers; operational
errors are not converted to Error cells. Arrays/raw byte strings cannot be
published merely by placing their constructor inside Eval. Native result subtypes,
NULL/coercion/error precedence and wider Variant behavior remain unqualified.

```sh
tablecraft query eval "SELECT Eval('15+3'),Eval('Chr$(65)')"
tablecraft query run "SELECT Eval('15+3') FROM Items GROUP BY Eval('15+3')" --database example.accdb
tablecraft query run "SELECT ID,Eval(CStr(ID) & '+17') FROM ScalarSamples ORDER BY ID" --database example.accdb --number-format invariant
```

## Name and host boundaries

The entire parsed shape is validated before executing it. Dynamic fields,
parameters, form/control members, identity expressions, SQL subqueries and
query-only domain/aggregate calls are refused. There is no dynamic name capture
from the current row, outer parameter scope or supplied scalar Bindings callback.
Ordinary outer arguments can explicitly construct text from bound fields,
parameters or supported domain calls. Those dependencies remain visible to binding
and source projection pruning.

Only the existing closed built-in set and explicitly supported Array constructor
forms are resolved. User functions, external providers, object/COM activation,
filesystem/process calls and VBA code are not invoked. This also prevents an
unselected dynamic field or an unknown function later in the parsed tree from
requesting host services. It is an explicit closed-evaluation contract, not a
claim to implement Access's full application expression host. Inspection/opening
native files never executes stored Eval formulas implicitly.

## Shared context, keys and limits

Nested dynamic calls execute through the same scalar evaluator. They retain its
number/date/character/week/casing policies, collation and LIKE mode, supplied
clock, caller-owned Rnd state and immutable Access session metadata. They neither create fresh budgets nor fetch
ambient clock/locale/random providers. Successful earlier random draws remain
consumed if later evaluation fails. Parsing and complete shape validation precede
calls into those providers.

Runtime group and pivot keys require repeatable dynamic expressions. Parsed
clock/Rnd calls, including through nested Eval, fail DynamicNotRepeatable before
their provider/state is used. The runtime restores its previous key context even
on error. Hidden Rnd calls while constructing a key through a domain also fail;
ordinary direct clock-key behavior retains its existing rules. Literal and
field-produced pure dynamic keys can participate in structural grouping and
grouped-projection validation. Dynamic clock group keys from Microsoft's broader
examples remain unsupported; this does not qualify native optimizer, collation
or invocation order.

When a group/pivot key's domain call initiates named-crosstab schema discovery,
the newly executed definition and its nested discovery dependencies inherit that
key's dynamic-repeatability context. A fresh evaluator cannot reset the guard and
execute a hidden Eval random/clock body in a crosstab cell. Validation fails before
those host callbacks or random draws, including after earlier pure cells. Failed
partial discovery is not published or retained into another execution.

Already completed materialized cells remain values: if ordinary relational
preparation or an earlier WHERE/domain operation populated the cache outside a
key context, grouping those cached values does not execute the original formulas
again or retroactively reject their provenance. Normal discovery retains its
existing evaluation counts. Context inheritance shares the same cancellation,
parameter rules and cumulative bytes/rows/steps allowances; it introduces no new
source scan or persistent cache.

DISTINCT projected-sort matching treats Eval as opaque, even when its argument
is literal pure text. Repeated argument syntax cannot prove a reusable result
without interpreting a dynamic body or inspecting execution values. A repeated
Eval expression in ORDER BY therefore fails metadata-only preparation; select an
explicit alias or ordinal to sort the already evaluated value. This also covers
Eval nested in larger expressions or aggregate arguments. Ordinary non-DISTINCT
repeated Eval expressions retain separate eager calls, while grouped pure Eval
keys retain the runtime repeatability checks above. Focused controls compare
selected random outputs and exact four/eight draw counts for these distinct paths;
they do not qualify native Access optimizer invocation counts.

Each dynamic input shares the caller's bytes/steps allowance and cancellation
callback. Source-sized syntax allocation/work allowances are charged before
parsing; they are logical resource prices, not an exact allocator-memory meter.
The parser separately bounds bytes, tokens and syntax depth, and its bounded
internal staging is not interruptible at every token. Cancellation checks bracket
parsing, precede shape visits and remain active during evaluation.

Text input/output remains capped by EvalLimits. The parser retains its full
65,536-byte input ceiling without a parenthesized guard copy. Dynamic nesting in
the scalar evaluator consumes its remaining 32-level expression depth; nested
Eval never resets it. Bound query trees retain their separate preparation-depth
ceiling, and dynamic bodies use the evaluator's bounded depth under the same
operation budget. Repeated calls consume cumulative allowances even after errors.
CurrentUser reads the supplied immutable session and remains admissible in
repeatable dynamic keys; it performs no OS or workgroup lookup. No dynamic AST
cache or new dependency was introduced. DynamicSyntax, DynamicLimit
and DynamicNotRepeatable errors carry no input payloads. CLI results remain fully
buffered on parsing, evaluation, cancellation and output failures.

## Evidence and remaining work

The strict parser has an independent full-byte-ceiling and terminator/string/
comment regression. Four scalar controls cover nested results, numeric arguments,
Error data, supplied clock/Rnd context, preflight name/effect refusal, shared
quotas, depth and every observed cancellation checkpoint. Four bound controls
cover fields/parameters, grouping/aggregates/correlation, crosstab pivot refusal,
source-predicate safety and shared quotas/cancellation. A CLI control exercises
both commands, fixed clock/random state and empty stdout on late failures.

The original [independent runner](testing/compare_dynamic_eval.py) passes 1084
integer/Boolean/civil/context/refusal/native-field controls. Reviewed getters
supply IDs from four unchanged original ScalarFixture files with expression
evaluation disabled. The runner does not execute Eval in Microsoft Access or
qualify saved/application formulas. Dynamic host names, supplied callable/UDF
contracts, forms/controls, wider regional/Variant semantics and native comparison
remain backend compatibility work or explicit VBA-host deferrals. Eval is Partial.
