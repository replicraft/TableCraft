# Explicit General comparison allowances

`query run --comparison-bytes BYTES --comparison-steps STEPS` supplies the
separate cumulative General-provider pool. Both flags require `--collation`.
Defaults remain 16 MiB and 1,000,000 steps; positive explicit ceilings reach
256 MiB and 100,000,000 steps. The controls apply to `database`, `general-legacy`
and `general-2010` providers for this run. Ordinal comparison keeps its existing
query-owned work profile and has no General provider pool to consume.

Database-mode field policies share one borrowed pool. Global General selection
owns one pool. Switching field policies or revisiting expressions does not renew
or refund it. Ordinary comparison, database-mode search and LIKE consume their
existing metered provider operations. Native reads/index interpretation, query
execution/copies, Text admission and final output remain independently bounded.
Increasing native or execution allowances cannot pay for exhausted comparison
work. A large ceiling does not preallocate its full byte allowance.

For example, an existing original corpus supports a larger sequential comparison:

```sh
tablecraft query run \
  'SELECT ID FROM TextIndexed WHERE Label=String(255,"a")' \
  --database .cache/tablecraft-fixtures/null-indexes-20261024/Text-V2016-1.accdb \
  --collation database --comparison-bytes 134217728 --comparison-steps 100000000 \
  --native-steps 100000000 --execution-steps 100000000 --format tsv
```

The explicit quotas raise resource allowances, not compatibility guarantees or
format maxima. Existing provider semantics and unknown-policy refusals remain.
This adds no ambient locale selection, Windows comparison evidence or write
qualification. Provider failures retain consumed work; results stay buffered.
Library hosts continue supplying their own ResourceBudget to GeneralCollation
or NativeCollationResolver. Other CLI commands keep their previous defaults.

Two focused CLI tests cover ranges, required policy selection, maximum acceptance,
low independent pools and empty stdout on failure. Nine existing General/database
CLI tests remain passing; scoped CLI Clippy, formatting and build pass. Native
Access and performance qualification remain separate.
