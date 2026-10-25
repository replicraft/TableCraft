# Original native recomputed-view replay

The [accepted report](results/recomputed-views-2026-10-25.json) passes 80 controls
on the registered ACE 2016/2019 calculated-dependency originals: 48 successful
queries and 32 intended buffered refusals. It compares 212 rows and 460 cells
with no disagreements. This is independent engineering evidence, not native
Access formula execution, freshness or save/reopen qualification.

[The runner](compare_recomputed_views.py) reopens both originals using the existing
inert Java getter with expression evaluation disabled and a throwing link
resolver. Fresh getter output must match the registered observations exactly.
Independent integer/NULL/String models use ordinary ID, Input and Label values;
raw stored calculated caches deliberately differ and are checked separately.

Both materialized and declared-arithmetic modes cover direct carrier inspection,
filtering, NULL grouping, native-table joins, numeric/Text UNION ALL, runtime
DLookup, derived expressions, empty populations and mixed fresh/stored queries.
Refusals cover eager selected errors under false filters/TOP, ordinary unavailable
calculated fields, captured aliases, missing fields and native/query/output limits.
Some admission refusals assert nonzero status and empty stdout only; typed error
classification has separate focused Rust tests.

The CLI was frozen from clean revision
`5292730e4fa98c0fa6622fbee8e9a38c08a5ae43`, SHA-256
`8f42fd5a936840e7dbb67800d0635305b625bea852fbb40be92641bf627b4811`.
All twelve immutable input/tool hashes and thirteen scoped compiled-source hashes
were independently rechecked. Original bytes remain unchanged. Source snapshots
come from that revision rather than concurrent live files.

The first run had four parser refusals because its numeric UNION used the
reserved field name Input without brackets. The exact failed runner, report and
log remain under ignored `.cache/recomputed-views-exploration-20261025/`.
Only the SQL selector changed to `[Input]`; no model, production code, original
or frozen executable changed. The accepted fresh replay uses the same originals.

Commands, outputs, expected rows and hashes remain in the report. Its runner pins
existing getter/model/display helpers separately and refuses to overwrite an
existing report. No new fixture binaries or dependencies were adopted; original
recipe and getter provenance remain in [FIXTURES.md](../FIXTURES.md).

[Focused tests](../RECOMPUTED_CALCULATION_VIEWS.md) separately cover invalid
caches, paired result types, retained Strings, provider reuse, metadata-only
preparation, exact cumulative limits and cancellation. These views remain explicit
buffered computations over selected formulas; they do not replace native table
routes, expand table-function admission, or establish complete read/query support.
