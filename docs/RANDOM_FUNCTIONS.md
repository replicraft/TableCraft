# Rnd query state

Rnd now returns Single results in scalar/bound queries and queries over explicit
native table sources. The [Access query sandbox list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
establishes direct query availability. The
[Microsoft Rnd specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d6cdff3d-627b-4203-9ea3-bca5def01444)
defines the argument-dependent sequence behavior; it does not prescribe an exact
Access generator. The implementation uses independently written arithmetic for
the recurrence/seed mapping reviewed in Microsoft's
[Basic reference implementation](https://github.com/dotnet/runtime/blob/main/src/libraries/Microsoft.VisualBasic.Core/src/Microsoft/VisualBasic/VBMath.vb)
and [initial state](https://github.com/dotnet/runtime/blob/main/src/libraries/Microsoft.VisualBasic.Core/src/Microsoft/VisualBasic/CompilerServices/ProjectData.vb).
No source text was copied. Native Access sequence and optimizer qualification
remain open; Rnd is Partial.

| Argument after checked Single conversion | Current behavior |
| --- | --- |
| Omitted or positive | Advance the caller's sequence once |
| Zero, including signed zero | Return the current state normalized to Single, without advancing |
| Negative | Derive state from its Single bits, then advance once; repeated identical negative values reproduce the same result |

Numeric carriers and [legacy DATE serials](DATE_NUMERIC.md) use the existing
checked Single conversion. SQL numeric Text additionally uses the evaluator's
explicit NumberFormat and shared conversion limits; the pure kernel does not
infer a locale. NULL, nonnumeric
Text, nonfinite values and out-of-range conversion fail before state mutation.
The resulting fraction is a 24-bit state divided by 2^24 and lies in `[0,1)`.
Initial Rnd(0) reflects the host-selected initial state, before any draw.
Optional bare syntax without parentheses and broad Variant coercion remain outside
the current parser/evaluator profile.

## Ownership and CLI

`RndState` lives with the numeric function kernel and owns one validated 24-bit
state in Cell. It is deliberately sequential and not Sync. It has no OS entropy,
clock lookup, process-global or thread-local storage. Hosts supply it through
Evaluator::with_random or ExecutionOptions::random. Binding and plan reuse do not
capture, create or advance state. Nested/subquery/domain evaluation uses the same
supplied execution state. Standalone callers without one receive
RandomStateRequired. MathFunction::apply_with_random supplies the same explicit
context to parser-independent consumers.

Each CLI eval/run command constructs one state. `--random-seed` accepts a raw state
from 0 through 16777215; omission uses the reviewed Basic initial state 327680.
The flag initializes raw sequence state and does not claim Randomize semantics.
Clap validates it before opening a database. Repeated fresh commands with the same
SQL and seed are reproducible; hosts can explicitly reuse a state across requests.

```sh
tablecraft query eval "SELECT Rnd(),Rnd(0),Rnd(-1),Rnd(0)"
tablecraft query run "SELECT ID,Rnd(-ID) FROM ScalarSamples ORDER BY ID" --database example.accdb --random-seed 42
```

`getrandom` already supplies OS entropy for encryption salts/verifiers. It solves
a different part of the problem: fresh seed generation can use OS entropy, while
Rnd must retain its repeat/advance/reseed behavior and Basic-compatible mapping.
No entropy-backed randomization mode is implemented by this flag, and Rnd state
must never generate password salts or verifiers. Randomize remains deferred as a
runtime statement; stored VBA is not executed to implement these query calls.

## Execution semantics and limits

Scalar operands and output columns evaluate in written order. Query draws follow
the existing engine pipeline: complete filtering precedes projection; joins,
aggregates, correlated queries and sorting have their existing execution stages.
Ordering by a projected alias reuses the projected samples. Safe source predicates
do not extract a partial AND that would suppress Rnd calls on discarded rows.
Native optimizer invocation counts are unqualified; source SQL text order does not
promise one universal draw order across clauses or native Access.

Direct Rnd group keys remain unsupported under the existing pure computed-key
contract. Materializing correct volatile keys instead of reevaluating them remains
query work. This restriction also catches Rnd nested inside a candidate key.
Ordinary projected/aggregate Rnd calls remain supported. Plans retain neither state
nor random values. Projection pruning/native optimizer equivalence still requires
qualification; there is no promise that unused expressions consume draws.

Cancellation and work charging precede mutation. Invalid arguments and refused
pre-call work do not consume a draw. Earlier successful calls remain consumed
when later query work fails; buffered CLI output is discarded, but host state is
not a database transaction and is not rolled back. No entropy retry, blocking
lock, heap allocation or unbounded contention loop occurs in a draw. Parallel
queries must use an explicit sequence-ownership policy rather than race this state.

## Evidence

Three kernel controls cover default/raw states, zero/negative behavior, invalid
inputs and a fixed-seed 256-case wide-integer modular model. Two scalar controls
cover every observed pre-draw cancellation checkpoint, provider absence, invalid
arity and work refusal. Three bound controls cover plan/state reuse, nested calls,
filters, aggregates, sort aliases and cancellation/work refusal. CLI controls
exercise both commands, raw-seed validation, repeatability and buffered errors.

The original [independent runner](testing/compare_random.py) passes 1680
state/seed/Single/refusal/native-field controls. Python uses unbounded integer
modular arithmetic and explicit IEEE Single packing; output comparisons are exact
at the Single-bit level. Native ID fields come from the four original
ScalarFixture databases via reviewed getters with expression evaluation disabled.
All file hashes remain unchanged. The initial query test expected projection to
interleave with filtering; inspection confirmed the existing filter-then-project
pipeline, and the corrected test checks that observed sequence without changing
execution. It is not native Access evidence.

Actual Access default seed, sequence/coercion/error details and optimizer call
counts, broader volatile-key semantics, Randomize/session integration and wider
native saved-query qualification remain open. No dependency or stored carrier was
added or changed.
