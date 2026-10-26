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
path length 16, cycle detection, schema widths and source allowances are unchanged.
Raising structural depth grants no additional consumable bytes, rows, output or
work. Text-policy and projection-pruning visitors also check cancellation and
charge the shared work budget during staged preparation; their independent
visit ceilings derive from the captured depth and node ceiling.

Out-of-range CLI values fail argument parsing before opening a database. Excessive
owned query depth or node count returns `StructureLimit` before row scans.
Buffered errors publish no partial results. Explicit streaming keeps its existing
[publication and late-failure contract](STREAMING_QUERIES.md).
The restricted editing planner retains its default depth.

## Verification

Focused query tests cover default refusal and explicit compound UNION/grouping,
derived/named sources, cycles, domain parsing, dynamic crosstab discovery,
required text policies, extrema, progressive group identity, absolute guards and
every observed execution/staged-preparation cancellation checkpoint. CLI tests
cover invalid arguments, both arithmetic profiles and deeper native domain and
progressive queries over the existing ACE 2007 example.

Selected collation, domain, named-crosstab, editing and progressive regression
suites also pass. These are engineering checks, not Microsoft Access comparisons
or completion of the broader read/query roadmap.
