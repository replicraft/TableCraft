# Explicit query binding depth

`query run --binding-depth DEPTH` selects structural planning depth from 1 through
32. The default remains 16. This applies to buffered and eligible streaming
queries, saved/named dependencies and execution-stage schema discovery. It allows
deeper compound queries without increasing the default profile.

For example, the following explicitly allows deeper planning while reading only
the selected stable/offline database:

```sh
tablecraft query run --database example.accdb --binding-depth 32 \
  'SELECT Count(*) AS N FROM (SELECT ID FROM Customers) C'
```

Depth measures planning helper calls, including query, relation and expression
structure, rather than counting parentheses. A query accepted by the SQL parser
can still exceed the binder's limit. This option does not broaden supported SQL
syntax or make unsupported streaming plans eligible.

## Library contract

`BindingDepth::new(usize)` validates the nonzero range. Its private representation
prevents constructing an unchecked selection; `Default` selects 16.
`PreparationOptions.binding_depth` captures the limit in `PreparedQuery`, exposed
through `binding_depth()`. Execution of that prepared plan retains its captured
limit even when `ExecutionOptions.binding_depth` differs. Runtime domain strings,
text-policy walks and progressive plan admission use that same captured limit.

`ExecutionOptions.binding_depth` selects preparation depth for
`execute_with_schema_discovery`. It does not reinterpret an already prepared
plan. To change a plan's limit, prepare it again. Preparation stays metadata-only;
data-dependent discovery remains an explicit execution phase.

## Independent limits and failure behavior

The scalar evaluator's absolute depth envelope remains 32. Parser ceilings,
2,048 combined planning nodes, 1 MiB retained-plan estimates, named dependency
path length fifty, cycle detection, schema widths and source allowances remain
independent. The named path ceiling does not reset the composed interpreter
allowance below.
Raising structural depth grants no additional consumable bytes, rows, output or
work. Text-policy and projection-pruning visitors also check cancellation and
charge the shared work budget during staged preparation; their independent
visit ceilings derive from the captured depth and node ceiling.

Out-of-range CLI values fail argument parsing before opening a database. Excessive
owned query depth or node count returns `StructureLimit` before row scans.
Buffered errors publish no partial results. Explicit streaming keeps its existing
[publication and late-failure contract](../cli/STREAMING_QUERIES.md).
The restricted editing planner retains its default depth.

## Composed interpreter nesting

Named definitions have a separate fifty-definition path ceiling. Plain SELECT,
TABLE, named UNION references, first- and later-arm UNION dependencies, and fixed
crosstab chains complete at fifty in isolated one-MiB debug worker probes.
A fifty-first definition and dependency cycles refuse before scans. Dynamic
crosstab and mixed inline/named probes instead reach a checked structural limit;
they are not qualified as general depth-fifty success.

The interpreter also has an engineering allowance of 128 active nesting units.
It is neither a native Access limit nor a measurement of Rust stack frames.
Active runtime reads share one operation counter, including fresh runtimes used
by execution-stage discovery. Runtime domain binding and staged preparation
inherit that active count. Pending named definitions retain their suspended
structural cost across each definition's local syntax-depth reset: a lean named
entry costs one unit, with additional nested syntax consuming additional units.
The named path, per-definition depth, node, byte and runtime domain-call limits
continue to apply independently. Domain-call nesting remains capped at sixteen,
with its counter shared across fresh discovery runtimes. Direct crosstab execution
also enters the read allowance; a crosstab already reached through a counted read
uses the internal path without double-counting.

This bounds repeated domain preparation over deep named sources without treating
identical invocations as cycles or changing volatile-query behavior below the
allowance. Entered reads and definitions restore their owner, parameter scope and
pending counters on errors. A deep named/domain probe and a domain-triggered
staged-discovery probe return checked limits on the one-MiB worker, rather than
exhausting its stack. These probes establish the recorded implementation envelope;
they do not establish every shape, platform or native Access execution outcome.
A separate staged-domain probe succeeds with sixteen active calls and two sequential
invocations, then refuses a seventeenth call before its target scans. It stays below
the named-fifty and active-interpreter allowances, distinguishing the domain guard
from those other limits.

## Verification

The named-depth scope passes 164 distinct tests: 102 named/domain/binding/source
controls, 47 execution units and 15 binding units, including fifteen new depth/probe tests and four
new scope-restoration units. The [scoped record](../testing/results/read-query-named-depth-2026-10-25.json)
separates successful fifty-definition shapes from checked compound refusals,
retains final command/log hashes, and records excluded initial stack aborts,
timeouts and test-construction/lint diagnostics. Native Access comparison remains
unperformed for these new deep-query shapes.


Focused query tests cover default refusal and explicit compound UNION/grouping,
derived/named sources, cycles, domain parsing, dynamic crosstab discovery,
required text policies, extrema, progressive group identity, absolute guards and
every observed execution/staged-preparation cancellation checkpoint. CLI tests
cover invalid arguments, both arithmetic profiles and deeper native domain and
progressive queries over the existing ACE 2007 example.

Selected collation, domain, named-crosstab, editing and progressive regression
suites also pass. These are engineering checks, not Microsoft Access comparisons
or completion of the broader read/query roadmap.

The independent [compound replay](../testing/DEEPER_QUERIES.md) adds 1,692 successful
explicit-depth queries and 141 identical-SQL default-depth refusals over literal
constructors and four unchanged native scalar originals. It checks exact retained
values, group memberships, ordering and extrema in both arithmetic profiles.
