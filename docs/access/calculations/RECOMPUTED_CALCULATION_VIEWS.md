# Explicit recomputed calculation views

`RecomputedNativeSource` exposes explicitly selected ordinary and calculated
fields as fresh named query views over an unchanged `NativeSource`. A view can
participate in filters, joins, grouping, sets, derived queries and parameterless
domain retrieval through the existing query executor. Original native table names
retain their existing ordinary or explicitly cached-read behavior.

```rust,no_run
use std::{fs::File, cell::RefCell};
use tablecraft_access::{CalculationView, NativeSource, RecomputedNativeSource};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

let pool = RefCell::new(ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000));
let native = NativeSource::new_with_shared_budget(
    File::open("offline.accdb")?,
    CatalogScope::new(1),
    &pool,
    &|| false,
)?;
let views = RecomputedNativeSource::new(
    &native,
    &[CalculationView::new("FreshOrders", "Orders", &["ID", "Total"])],
    ParseLimits::default(),
)?;
let sql = parse_query(
    "SELECT Total,Count(*) FROM FreshOrders GROUP BY Total",
    ParseLimits::default(),
)?;
let plan = prepare(sql.query(), &views)?;
let result = plan.execute(&views, &[], ExecutionOptions::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Selected expressions and metadata

The host supplies an alias, one local base table and an ordered field list.
Selected fields keep their canonical native names. The constructor retains a
checked owned SELECT definition; formula text is never interpolated into SQL.
Aliases cannot collide with any captured table or saved-query name, including
saved definitions whose contents are unsupported. Names use the source's existing
ASCII-insensitive lookup. Duplicate aliases and selected fields refuse.

There are at most 256 views, each selecting 1..=256 fields. Names are bounded to
255 UTF-8 bytes. This first implementation retains the preview's checked bracket
identifier boundary: closing brackets and control characters refuse. Base tables
must be local; this service opens no paths, providers or linked targets.

Calculated fields reuse the existing
[calculation declaration and dependency profile](CALCULATION_PREVIEWS.md).
Only its existing operators and exact three-present-argument unqualified `IIf`
are admitted. Missing fields, unsupported calls, invalid declarations, cycles and
dependency limits fail before user records, including empty tables and unused
IIf arms. Unselected calculated declarations remain inert.

Microsoft's [table-expression example](https://support.microsoft.com/en-us/office/add-functions-to-access-expressions-72a471a0-4f0e-4d49-923b-be97b0fa4bb0)
establishes the existing narrow `IIf` table-calculation context. General query
function availability does not enlarge this formula whitelist.

## Natural results and eager view boundaries

Every selected calculation and its calculated dependencies use their natural
expression results. The native stored ResultType does not narrow, format or
convert intermediate values. A stored LONG declaration can recompute an Int64 or
Text. The existing named-query binder derives nullable, paired arithmetic result
descriptors and operand categories from expressions rather than manufacturing
stored-column metadata. Preparation and execution options retain their existing
independence. Formula literals follow the caller's preparation options; the
standalone preview continues to capture its existing default literal profile.

Views use existing derived-query execution. All selected view projections execute
before the outer query's WHERE or TOP. Selecting fewer outputs or filtering every
row outside the view does not suppress an error in another selected expression.
Choose a narrower field list when constructing the view to authorize less work.
Repeated references follow existing derived-query behavior and can recompute;
this service makes no global or per-file memoization promise.

Only ordinary leaf dependencies become native input projections. Stored cached
calculated values are never operands, even when the underlying source separately
enables cached reads. Corrupt unselected cache wrappers can coexist with successful
recomputation; selected ordinary fields and row geometry still validate normally.
Native field policies, fresh expression policies and retained intermediate cells
flow through the ordinary evaluator. Comparers and formatting policies activate
only at their existing consumers.

## Resource and preservation boundaries

Construction and ordinary query preparation are metadata-only. They do not read
user records, evaluate formulas or activate saved content. Formula/dependency
syntax, view names, checked definitions and lookup work consume the borrowed
source's cumulative native allowance with cancellation and fallible allocation.
No source pool is replaced, reset or refunded.

Execution uses one caller-owned query Runtime and its normal byte, row, work and
Text limits. Native reads retain their separate source allowance. No nested
per-row evaluator, independent formula budget or partial-result cache is created.
Original Source metadata, projected/constrained scans, collection reads and policy
callbacks retain their native implementation. The unchanged/offline source
precondition remains required; this wrapper supplies no live-file snapshot.

Ordinary execution remains buffered: a late formula, source, resource or
cancellation failure publishes no query result. Explicit progressive execution
can consume these static row-local views without retaining a complete input
snapshot. It evaluates every selected formula before outer filters and retains
intermediate cells until their consumers. Earlier complete output batches can
survive later failures; no prefix becomes a complete relation cache. The original
file is never modified. Successful recomputation proves
neither cache freshness nor native Access formula equivalence and enables no
write, save, validation-macro or recalculation policy.

## Explicit CLI selection

`query run --recomputed-column VIEW TABLE COLUMN` selects one ordered output
field in a fresh local query view. Repeat the option with the same view/table
to select more fields. The CLI admits at most 256 selections total; the adapter
validates canonical names, duplicate outputs, object collisions and formulas.

```sh
tablecraft query run --database example.accdb \
  --recomputed-column FreshItems Items ID \
  --recomputed-column FreshItems Items Score \
  'SELECT ID,Score FROM FreshItems WHERE Score>10'
```

Different aliases can select separate views. Reusing an alias with a different
base table fails. Ordinary fields can be selected alongside calculated fields;
only the explicit output list is authorized. Existing table names and
`--stored-calculations` continue to address original stored values.

This CLI composition is local. It supports explicit `--stream` for eligible
static row-local pipelines and otherwise stays buffered. SQLite, linked/external
target mappings and template sources remain unavailable. Inner aggregate/grouped,
UNION, ordered, distinct or discovery-dependent inputs are refused by the
[streaming profile](../../cli/STREAMING_QUERIES.md#static-named-and-derived-inputs); outer
admitted aggregates and root flat UNION ALL remain available. The same password, comparison/formatting context, arithmetic preparation profile and
separate native/query/output limits apply. Natural recomputed output may differ
from the native stored ResultType; no automatic cast or freshness qualification
is implied. Selected view expressions remain eager before an outer WHERE/TOP,
even if the outer query selects fewer view columns. Narrow the explicit view
selection to leave an unrelated formula inert.

## Verification

Five implementation tests, seven independent conformance tests and twenty-one
existing native preview regressions pass. Nine new CLI tests and sixty-eight
CLI regressions also pass, for 110 unique scoped tests. Access and CLI test-aware
strict Clippy, workspace formatting and dependency boundaries pass. See the
[verification record](../../testing/results/read-query-recomputed-views-2026-10-25.json)
for exact commands, counts and retained failed attempts. An initial constructor
Field/Expression projection mismatch was fixed without weakening tests; an
independent overflow golden was separately corrected to the established Long
to Double rule after a pre-feature baseline check. Native Access formula
execution/save/reopen and original-file replay remain separate qualification.

An [original native replay](../../testing/RECOMPUTED_VIEWS.md) additionally passes
80 getter/model controls over immutable ACE 2016/2019 files, comparing 212 rows
and 460 cells. Twelve immutable pins and thirteen scoped compiled-source hashes
were independently rechecked. Native Access qualification remains separate.
