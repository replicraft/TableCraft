# Original DATE query candidate replay

The [192-command report](results/date-query-candidate-replay-2026-10-25.json)
passes over four original 20,000-row databases: Jet 4, ACE 2007 and ACE 2016/2019.
There are 172 successful queries with 539,848 compared ID rows and twenty expected
buffered failures. Every original and five artifact/source pins remain unchanged.
This is independent engineering evidence, not Microsoft Access SQL qualification.

## Inputs and independent model

The original [Java recipe](DateCandidateFixture.java) uses the reviewed Jackcess
5.0.4 JAR to create fresh scalar tables with ascending and descending DATE indexes.
Each table has an ID permutation, NULLs, both signed zeros, positive/negative epoch
aliases, positive/negative minimum subnormals, negative-date fractions, later dates
and year-envelope boundary values. Both indexes have internal node roots. No VBA,
queries, links, macros or other active content is introduced. The generator disables
expression evaluation and automatic per-row sync; normal close completes the file.

Read-only getters return table and index encounter order plus exact stored DATE
bits. The retained Jackcess DATE carrier is serialized into an eight-byte buffer;
calendar conversion and `toDateDouble` would lose sub-millisecond information.
The runner checks required raw aliases/subnormals and complete ID membership,
then maps each finite IEEE value into an exact Python `Fraction`. Calendar keys
are integer truncation toward zero plus the absolute fractional remainder. This
does not call TableCraft, use its candidate-bound algorithm or ask Jackcess to
evaluate SQL. Independent index cursors supply tie/reference order directly.

## Controls and limits

Sequential, automatic, required ascending and required descending reads compare
exact parsed TSV cells for epoch equality, midnight, negative points, half-open
negative ranges, reversed operands, empty ranges, NULL/non-NULL and contradictory
NULL intersections. Typed CLI DateTime parameters cover epoch and negative dates.
Automatic answers preserve sequential encounter order; required indexes preserve
independent physical key order. Most comparisons use identical explicit larger
allowances so the complete sequential baseline can load all 20,000 rows.

Separate default-limit controls successfully select four small populations while
an unconstrained full count fails. A wide non-NULL candidate set still fails at
the unchanged snapshot ceiling. A nested complete count after a selected outer
population also fails: a selected prefix cannot masquerade as a complete cache.
An unsafe compound predicate retains division by zero. Corrupt chosen roots on
disposable copies fail without fallback. All twenty expected errors have empty
stdout; only copies receive mutations. Recorded disposable paths use a portable
placeholder rather than machine-specific temporary paths.

The frozen debug CLI has SHA-256
`e80e680c42646eb712696c8b14405d279682a3769f2f0393d94358f252c64064`,
built from `3b1e2344617b8dfffe14cf556e33bc8deff7f5f1`. Neither product code nor
successful expected rows changed during replay. The first attempt rejected a
correct failure because of a runner diagnostic wording mistake; its record is
preserved. An earlier 184-control pass is retained separately from the final
192-control superset. Counts do not add repeated runs together.

```sh
python3 docs/testing/compare_date_candidates.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli .cache/tablecraft-bin/date-query-cli-20261025 \
  --directory .cache/tablecraft-fixtures/date-candidates-20261025 \
  --report .cache/date-candidate-new-replay.json
```

`--generate` requires all four destinations to be fresh and never overwrites
existing originals. Reports must also be fresh. The JAR and generated databases
stay ignored; public recipes and [fixture provenance](FIXTURES.md#original-date-candidate-inputs)
are sufficient to recreate independent inputs. Runtime/compiler resources are
temporary and are not product dependencies. This replay does not establish native
Access optimizer behavior, complete collation/Variant semantics, writes or live
shared-file coherence, and does not complete the entire read/query roadmap.
