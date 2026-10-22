# Inert saved-query fragments

`read_saved_query_definition` now reads one explicitly named query's stored
MSysQueries fragments from a stable/offline source. It returns an owned
`SavedQueryDefinition` with its scoped model object, unmodified native origin and
selected `SavedQueryRow` records. It does not reconstruct, validate or execute SQL,
activate a provider, evaluate expressions or start application content.

The catalog lookup requires `ObjectKind::Query`. Native signed object IDs match
MSysQueries.ObjectId exactly, including negative IDs; they are not table/page
addresses and receive no page-number masking. Caller-issued catalog scopes remain
separate from native IDs. A table or absent name reports UnknownObject; an existing
query with no stored rows returns an explicit empty definition.

## Stored field profile

The current system-table profile requires exactly eight named scalar fields:

| Native field | Retained carrier |
| --- | --- |
| ObjectId | Int32 matched exactly to the selected native query ID |
| Attribute | Nullable Byte mapped to QueryAttribute, including Unknown(code) |
| Expression | Nullable owned Text |
| Flag | Nullable signed Int16, without defaulting or interpreting bits |
| LvExtra | Nullable signed Int32 scalar, separate from object payload streams |
| Name1 / Name2 | Nullable owned Text, without identifier/path interpretation |
| Order | Nullable owned Binary, without sorting or coercion |

Native attribute codes 0–11 describe start/type/parameter/flags/remote-database/
table/column/join/filter/group/having/order fragments; 255 is the end tag. Every
other byte remains an explicit unknown tag. NULL, empty text, literal `NULL`,
empty Binary, signed minima and unknown flags remain distinct. Rows retain
sequential storage order in the engine and raw inspector. Explicit reconstruction
uses a separate [ordered view](NATIVE_SAVED_ORDERING.md).
Strings and binary buffers move out of the validated scalar snapshot without
cloning their payloads. Formatting remains at the executable boundary.

The reader scans the complete bounded system table before selecting its rows,
so other queries also consume the caller's read/row/work/allocation allowances.
Selected row-vector allocation is charged and reserved fallibly before conversion;
each scan/field visit checks bounded work and cancellation. Native geometry,
declared row counts, scalar decoding and schema errors publish no definition.
Normal reader ceilings and the qualified unencoded Jet 4/ACE 2/5/6 selector remain.
ACE 16-family original fixtures are the main qualification target; the existing
ACE-code-2 application supplies a separate observed regression case.

This is metadata inspection, not query/index integrity validation. The reviewed
template has a required composite ObjectId/Attribute/Order index. Nullable getters
preserve inspectable stored fields even when those semantic constraints are broken.
Rows without the selected ObjectId are omitted without being coerced to zero;
global orphan fragments, required headers, duplicate clauses, type/flag consistency,
dependencies and executable query completeness are not audited. Empty/unknown/
incomplete metadata does not authorize SQL execution or a later write.

## CLI

```sh
tablecraft inspect copy.accdb --query 'Saved Query Name'
tablecraft query saved --database copy.accdb 'Saved SELECT Name'
```

`--query` conflicts with all table/catalog/relationship/check modes. The report
buffers query name, native ID, raw flags, fragment count and the seven remaining
fields before stdout. Text is quoted/escaped; NULL is unquoted, empty Text is `""`
and empty Binary is `0x`. Borrowed text/binary formatting retains the existing
1 MiB output cap without cloning payloads. Errors exit 2 without partial output;
verbose diagnostics omit names, paths and expressions.

`query saved` separately prints normalized SQL from the qualified owned SELECT
adapter below, the [saved UNION adapter](NATIVE_SAVED_UNIONS.md), or the
[saved crosstab adapter](NATIVE_SAVED_CROSSTABS.md). It requires both an explicit native path and query name; it does
not bind or execute the result. The same native host budget covers reading and
adaptation. A fallible 1 MiB buffer completes formatting before stdout. Unknown/
unsupported/conflicting metadata fails with exit 2; parameter/index/SQLite/write
options are unavailable. `inspect --query` remains the raw fragment path for other
query families or unsupported reconstruction profiles.

`query run` accepts its documented scalar table sources and explicitly referenced
qualified SELECT/UNION definitions under the named-source contract below. Upstream parser ASTs remain inside the SQL adapter.
Action, DDL, pass-through and unknown fragments are data in this inspection mode
and never run.

## Owned SELECT reconstruction

`tablecraft-access::reconstruct_saved_select` consumes an explicitly read definition
and returns `SavedSelect`: its unchanged original fragments/identity and an owned
`tablecraft_sql::query::Query` containing declarations and one SELECT retrieval
statement. This opt-in adapter performs no binding, source
expansion, provider access or execution. The native engine retains storage facts;
the Access adapter owns their mapping into syntax. SELECT owner-access is a
retained syntax flag, never a grant of authority.

The current profile supports up to sixteen local named sources/aliases in bounded
[INNER/LEFT/RIGHT join forests and comma products](NATIVE_SAVED_JOINS.md), including
[stored derived SELECT/UNION sources](NATIVE_SAVED_DERIVED.md). It retains ordered
expression projections/aliases, explicit `*` or a flag-appended wildcard, WHERE,
GROUP BY, HAVING and ordered ASC/DESC sort expressions. DISTINCT, DISTINCTROW,
TOP (including zero), TOP PERCENT and owner-access remain separate owned fields.
TOP requires an ASCII u32 spelling; PERCENT requires TOP and a count at most 100.
Simultaneous DISTINCT/DISTINCTROW, unknown flag bits and conflicting counts fail.

Native query-object family bits must identify SELECT. A present type row must
agree; omitted type/start/end rows follow the observed reference profile rather
than becoming synthetic stored rows. Duplicate singleton clauses fail. Unknown/
NULL attributes, unused non-NULL extra/name/expression metadata, nonzero unhandled
row flags, unqualified parameter types/names, excessive join graphs, remote-source fragments and action/DDL/pass-through/
union/crosstab families fail explicitly. A table expression with NULL Name1 is a
guarded derived read; an expression with a named table remains an unsupported
remote qualifier. Literal object identifiers must be nonempty and exclude controls,
dot/bang/grave-accent and bracket content. One complete
[stored bracket delimiter pair](NATIVE_SAVED_NAMES.md) now normalizes to the literal
owned name; qualified stored forms remain unqualified. Expression text uses the existing owned parser, including
unresolved calls and read subqueries. This does not qualify those expressions for
execution or resolve nested sources.

All reconstruction families share [native fragment ordering](NATIVE_SAVED_ORDERING.md):
when every row has an Order value, sort by attribute and unsigned key bytes,
preserving physical ties; any NULL key preserves the entire physical sequence.
Original rows/keys remain unchanged. Empty/prefix/non-four-byte keys retain their
byte meaning within native carrier limits; they are not coerced to counters.

Expressions must parse both as stored and inside adapter-supplied parentheses
with a newline before the closing delimiter, yielding the same owned expression.
This rejects malformed parentheses that guard delimiters could otherwise repair.
Clause/list/statement injection fails; a trailing line
comment cannot consume the delimiter. Aliases become owned Name values separately.
No string concatenation can turn a stored alias into a new clause. The SQL parser's
new consuming `ParsedSelect::into_query` moves owned names without AST cloning.

The caller supplies ResourceBudget, cancellation and ParseLimits. Expanded
expression/name/type spellings share the clamped byte ceiling; each parser call also
enforces its token/depth ceiling. Vector/string reservations are fallible and
charged beforehand. AST/name allowances are charged before bounded parsing;
they are not an allocator quota for upstream temporary buffers. Fragment visits
consume rows/work, and cancellation is checked between bounded parsing calls and
before publication. Original fragments remain unchanged; failures publish no model.

An engineering host exposes the current API without executing its output:

```sh
cargo run --locked -p tablecraft-access --example saved_select -- \
  copy.accdb --query 'Saved SELECT Name'
```

Its normalized output buffers under 1 MiB before stdout. A saved source name is
still unresolved during reconstruction; explicit retrieval uses the named-source
contract below.

The original [SELECT recipe](testing/SavedSelectFixture.java) creates thirteen
inert definitions per V2016/V2019 family. The [read-only rendering oracle](testing/SavedSelectOracle.java)
uses Jackcess toSQLString with expression evaluation disabled, without executing
queries. [Comparison](testing/compare_saved_selects.py) normalizes those independent
renderings through the existing SQL parser and compares the adapter's complete
formatted models through both the engineering host and public CLI. Five disposable
corruption variants per family fail without stdout through both hosts; three
ordering copies now compare positively against the independent renderer. Seven Rust integration tests include a 128-case Unicode alias/literal
model, full owned-model equality, flags, boundaries, budgets and late cancellation.
Four additional CLI tests cover required/rejected options, normalized SQL with an
unresolved function, private diagnostics, atomic failures and the original example.

The registered ACE-code-2 example separately proves retained source identity for
an omitted-type query. The reference renderer emits `101f_cbo_Crew_Id_qry` bare;
that spelling is outside TableCraft's current bounded SQL grammar. Its exact
rendering is asserted separately, and the adapter is compared with an explicitly
quoted expected source name. This is not another whole-renderer normalization
result or native Access syntax qualification. [Provenance](FIXTURES.md#original-ace-16-saved-select-inputs)
records the original hashes. Actual Access and application save/reopen remain unrun.

### Native joins

The original two-source profile below now extends to
[bounded join forests and disconnected sources](NATIVE_SAVED_JOINS.md).
A typed join connects two explicit declared endpoints, with
one or more qualified native condition fragments.
Native kind codes 1/2/3 map separately to INNER/LEFT/RIGHT. Name1 is the left
endpoint and Name2 the right; source identity uses the declared alias, or the
literal table name when no alias is supplied. Endpoint orientation controls the
join even when table fragments appear in reverse storage order. Self-joins need
distinct aliases. Missing/ambiguous endpoints fail rather than creating implicit
sources. Disconnected declarations remain comma roots in stored construction
order. The declarations themselves remain unresolved database source names.

The current endpoint comparison profile is ASCII case-insensitive. Unicode table
names work with ASCII aliases; non-ASCII join labels await their own identity
qualification. This is metadata matching, not a data collation. Conditions pass
the independent original/guarded expression checks before bounded SELECT parsing
constructs owned Join nodes. Every resulting root, child, kind and condition is
checked before its owned source vector moves into the model. Parser allocations
retain the documented allowances/ceilings; cancellation precedes publication.

Three additional focused tests cover all kinds, both endpoint/storage orientations,
self-joins, Unicode table names, missing/ambiguous/unsupported metadata, expression
boundaries, budgets and late cancellation. The original recipe's `--joins` mode
adds nine joins to the thirteen baseline SELECTs. [Independent comparison](testing/compare_saved_joins.py)
matches all twenty-two models per V2016/V2019 family through both hosts. Eight
join mutations per family retain inert inspectability but reject reconstruction
without stdout: unknown/NULL kind, NULL endpoints/condition, undeclared endpoint,
ambiguous aliases and unqualified label comparison. [Provenance](FIXTURES.md#original-ace-16-saved-join-inputs)
records file/model hashes; no query execution or actual Access comparison has run.

#### Multiple conditions on the same pair

Up to sixteen native join fragments may describe a forest, including multiple
conditions attached to the same join. Endpoint matching is ASCII case-insensitive.
Later same-pair conditions may reverse their endpoint labels without reversing
the established outer join; matching kind and containing subtree govern placement.
NULL fields, conflicting kinds, unknown labels and unsupported metadata fail.
This is a qualified consumer ceiling, not a universal Access limit. Join fragments
follow the shared native key ordering/whole-definition NULL policy.

The reference prepends each subsequent same-pair condition. The adapter therefore
constructs the conjunction in reverse ordered-fragment order, preserving each
expression's parentheses and the resulting owned AND association. It does not
deduplicate predicates or apply a second predicate-specific sort. Each raw expression passes original/guarded
validation before joining text; bounded parsing verifies the complete conjunction
and source roots. Every condition borrows its original text. Multi-part
reference-vector/string reservations charge before allocation, and cancellation
is checked during construction and before publication. No condition executes.

Two focused tests cover all kinds with two/three distinguishable conditions,
invalid metadata/expression boundaries, the 16/17 ceiling, budgets and late
cancellation. The original `--compound-joins` mode adds six definitions to the
twenty-two baseline/simple-join controls. [Comparison](testing/compare_compound_joins.py)
matches twenty-eight independent models/family through both hosts. Five mutations
retain inert inspectability but reject reconstruction without stdout: mixed/NULL
kinds, NULL endpoints and undeclared condition endpoints. Three positive copies
check reversed same-pair endpoints and tied/reversed ordering keys against the
independent renderer through both hosts. Tied keys deliberately disagree with
the system primary index and are reader controls, not native-save qualification. [Provenance](FIXTURES.md#original-ace-16-compound-join-inputs)
records hashes. These are syntax/order results, not native execution/error-order
or actual Access qualification.

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java compound-2016.accdb V2016 --compound-joins
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java compound-2019.accdb V2019 --compound-joins
python3 docs/testing/compare_compound_joins.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --example target/release/examples/saved_select \
  --ace2016 compound-2016.accdb --ace2019 compound-2019.accdb
```

### Native parameter declarations

Ordered Parameter fragments now become the complete Query model's declarations.
`SavedSelect::query()` returns that Query rather than the earlier SELECT-only
view. Final sorting and owner-access occupy the same outer ReadQuery fields as
ordinary SQL parsing. A bounded parser-created statement carrier receives the
owned SELECT and parameter vector by move, without new unchecked Box allocation
or AST cloning. The consuming `ParsedQuery::into_query` accessor supports this
working adaptation. Neither reconstruction nor `query saved` binds values or
displays prompts; callers may explicitly use the existing query binder.

The qualified flag mapping is Boolean/Byte/Int16/Int32/Currency/Single/Double/
legacy DateTime/Binary/Text/LongBinary/GUID (native tags 1–11 and 15). It preserves
IEEESINGLE/IEEEDOUBLE syntax separately. Text LvExtra NULL/zero retains unspecified
TEXT syntax, and a positive value becomes its integer argument. Negative Text
lengths and any non-NULL extra on another type fail. These are syntactic arguments,
not stored-field width or native runtime-value qualification. Raw NULL and zero
remain distinct in the original fragments.

The parameter-name profile is literal ASCII, including
[punctuation-bearing prompt text](PARAMETER_PROMPTS.md). A stored outer bracket
pair is accepted; raw qualified control references remain unsupported. ASCII
case-insensitive duplicates compare the normalized name.
Declarations consume the shared ordered view without changing original rows. Missing names/type flags, unused expression/Name2 fields and unknown
types fail without defaults. VALUE/Variant, Memo, newer numeric/date tags and other
unqualified mappings remain unsupported. The 255-declaration ceiling matches the
existing bounded SQL parser policy, rather than asserting a universal engine limit.
Each declaration/name comparison charges work and checks cancellation; names,
types, argument vectors and boxed statement parsing retain the common allowances.

Four additional integration tests cover twelve mappings, text argument distinctions,
full owned-model equality, exact carrier binding through an explicit memory-only
host, metadata/identity rejection, native ordering, 255/256 declarations, budgets and late
cancellation. The synthetic fixture now packs bounded rows across data pages for
that ceiling test. A CLI test prints declarations without supplied values/prompts,
rejects unknown types and verifies payload-free diagnostics and unchanged input.

The original recipe's `--parameters` mode adds eight parameterized definitions to
thirteen baselines. [Comparison](testing/compare_saved_parameters.py) matches all
twenty-one models per V2016/V2019 family through both hosts, including prompts,
twelve type tags, NULL/zero/positive/max-i32 Text syntax, owner-access/sorting and
a parameterized join. Seven inspectable mutations/family reject reconstruction
without stdout; a tied-order copy compares positively through both hosts. [Provenance](FIXTURES.md#original-ace-16-saved-parameter-inputs)
records hashes. Actual Access, native query execution and runtime Text-value limits
remain unqualified; explicit saved/named retrieval is described below.

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java params-2016.accdb V2016 --parameters
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java params-2019.accdb V2019 --parameters
python3 docs/testing/compare_saved_parameters.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --example target/release/examples/saved_select \
  --ace2016 params-2016.accdb --ace2019 params-2019.accdb
```

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java joins-2016.accdb V2016 --joins
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java joins-2019.accdb V2019 --joins
python3 docs/testing/compare_saved_joins.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --example target/release/examples/saved_select \
  --ace2016 joins-2016.accdb --ace2019 joins-2019.accdb
```

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java select-2016.accdb V2016
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedSelectFixture.java select-2019.accdb V2019
cargo build --locked --release -p tablecraft-access --example saved_select
python3 docs/testing/compare_saved_selects.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --example target/release/examples/saved_select \
  --ace2016 select-2016.accdb --ace2019 select-2019.accdb \
  --sample docs/examples/Access_Example_VBA.accdb
```

## References and evidence

The reviewed Jackcess 5.0.4 [QueryImpl.Row source reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
and [QueryFormat reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryFormat.java)
provide native field/carrier/tag facts. Actual artifact bytecode and public
metadata getters were checked; no upstream implementation was copied.
SELECT clause/flag mappings also follow the reviewed artifact's
[BaseSelectQueryImpl reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/BaseSelectQueryImpl.java)
and QueryImpl getters. Stored ordering and syntax qualification remain limited
to the profile and observed comparisons above.
Parameter mappings/length facts were checked against QueryImpl's parameter
formatter and QueryFormat.PARAM_TYPE_MAP in the reviewed artifact.
[Microsoft's PARAMETERS declaration](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql)
defines declaration/name/prompt syntax; its runtime prompting behavior is not
implemented by inert reconstruction. The [type comparison](https://learn.microsoft.com/en-us/office/vba/access/concepts/error-codes/comparison-of-data-types)
distinguishes query parameters and SQL spellings from table field types.

[The original fixture recipe](testing/SavedQueryFixture.java) writes inert system
rows into fresh V2016/V2019 databases through the reviewed artifact. Eleven
definitions/75 fragments per family cover all nine query-object flag families,
unknown attributes, signed flags/extra fields, Unicode/NUL/control text, NULL/
empty distinctions, nonmonotonic order bytes and an empty definition. These are
stored fragment fixtures, not a claim that the illustrative SQL is executable.

[The read-only oracle](testing/SavedQueryOracle.java) uses query identity metadata
and raw stored rows with expression evaluation disabled; it never invokes
toSQLString or query execution. [Comparison](testing/compare_saved_queries.py)
matches all 75 fragments per family and all 59 queries/332 fragments in the
unchanged registered example. Unknown-tag, NULL-attribute, NULL-order and unbound
row copies still match inert inspection. Incompatible carrier/late-count copies
fail without stdout. [Provenance](FIXTURES.md#original-ace-16-saved-query-fragment-inputs)
records original file/report hashes and current limits; no native Access oracle
or application save/reopen has run.

Five native integration tests cover signed/scoped identities, namespace rejection,
unknown/nullable fields, empty definitions, exact carrier failures, geometry,
budgets/cancellation and a 128-case generated representation model. A unit sweep
round-trips all 256 attribute bytes. Three CLI tests verify inert exact output,
NULL quoting, payload-free diagnostics, every conflicting mode and buffered late
failures. The shared output refactor retains existing value formatting tests.

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedQueryFixture.java new-2016.accdb V2016
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/SavedQueryFixture.java new-2019.accdb V2019
python3 docs/testing/compare_saved_queries.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --ace2016 new-2016.accdb --ace2019 new-2019.accdb \
  --sample docs/examples/Access_Example_VBA.accdb
```


## Explicit saved SELECT execution

```sh
tablecraft query saved 'AllItems' --database copy.accdb
tablecraft query run --saved 'AllItems' --database copy.accdb
tablecraft query run --saved 'ParameterJoin' --database copy.mdb --param pFloor=long:1
```

`query saved` remains inert reconstruction and prints SQL only. `query run --saved`
explicitly reads, reconstructs, prepares and executes one saved SELECT/UNION/crosstab;
SQL text and `--saved` are mutually exclusive, and `--database` is required.
SQLite and implicit startup/query execution are not enabled. Existing `--param`,
`--collation`, `--like-mode`, mandatory `--index TABLE INDEX`, output formatting
and execution limits apply without a separate evaluator. All work finishes before
stdout. Metadata, parsing, binding, parameter, source or formatting failures return
no partial result and diagnostics omit saved names, SQL and values.

Shared `NativeSource::read_saved_select(&mut self, name, ParseLimits)` returns
`SavedSelect` with original fragments and owned syntax; it performs no execution.
Metadata reads, reconstruction and later native schema/table reads share the
same source, cumulative ResourceBudget and cancellation callback. Hosts can pass
`saved.query()` to `tablecraft_query::prepare`, then execute against that source.
`NativeSavedSelectError` distinguishes native reading and reconstruction errors.
The host must provide a stable/offline source; this does not create a snapshot
or native shared-file transaction.

Successful execution requires the intersection of the reconstructed metadata
profile and [bounded retrieval](QUERY_EXECUTION.md). A reconstructable query can
still fail preparation: owner-access, DISTINCTROW, TOP 0, UDFs and other unsupported
expressions remain rejected. Named local saved SELECT/UNION sources now resolve under the bounded dependency
contract below; linked sources and implicit parameter prompting remain unavailable.
No owner flag grants authority and no application event/provider/VBA is activated.
Text comparisons still require an explicit policy; LIKE additionally requires
an explicit wildcard mode. Native ANSI/locale settings are not inferred.

Focused synthetic tests cover same-source parameter/filter/order execution,
all four qualified layout families, original fragment retention, cumulative
budget exhaustion, cancellation, buffered refusal, diagnostic payload omission
and both CLI presentation modes. Independent fixture/model comparisons are
recorded in [fixture evidence](FIXTURES.md#explicit-saved-select-execution-inputs);
they remain separate from actual Microsoft Access SQL qualification.


## Nested saved SELECT sources

```sh
tablecraft query run "SELECT S.ID FROM [AllItems] AS S" --database copy.accdb
tablecraft query run --saved SourceIsQuery --database copy.accdb
```

NativeSource now captures inert query identities/names beside table entries and
implements the query library's optional named-definition metadata contract.
Only referenced definitions are read/reconstructed; complete definitions are
cached for repeated bindings/aliases. Cache loading and lookup use the same
cumulative native budget/cancellation controls as catalog, schema and data reads.
Expected scoped native IDs must still agree on load; ambiguous table/query names
fail explicitly. User records remain unread until execution. Neither inventory,
`inspect` nor inert `query saved` recursively executes definitions.

Both SQL input and explicit `--saved` execution can use qualified saved local
SELECT/UNION sources in FROM, joins, derived queries, expression subqueries and supported
UNION references. Original names/aliases remain qualifiers. Native reconstruction
still has its documented join/fragment limitations; [native UNION reconstruction](NATIVE_SAVED_UNIONS.md)
now uses a separate family adapter. No implicit parameter inference is added. Owner flags, action/pass-through
queries, UDFs, external sources and unsupported forms do not become executable.

The [named-source binder contract](QUERY_EXECUTION.md#named-retrieval-sources)
retains per-definition parameter scopes, merges only equal validated descriptors,
and rejects cycles, conflicts and unknown references before scans. Definitions
become owned operators at preparation. NativeSource caches are immutable: changed
metadata requires a new source from a fresh stable input and preparation again.
Stable/offline source ownership remains mandatory. No update-through-query support
or write qualification is introduced.

Original fixtures and independent getter/SQL-assembly comparisons are recorded in
[dependency evidence](FIXTURES.md#saved-select-dependency-inputs). Microsoft references
for the underlying stored-query and typed parameter concepts are
[QueryDef](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/querydef-object-dao)
and [PARAMETERS](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql).
They do not prove the new scope/merge profile matches native Access. Native
comparison remains unrun; inert independent metadata/getters and synthetic checks
remain separately identified.

## Saved read entry point

[Native saved UNION reads](NATIVE_SAVED_UNIONS.md) now share SELECT's explicit CLI
and named-source paths through reconstruct_saved_read / NativeSource::read_saved_query.
SavedRead/SavedReadError/NativeSavedReadError are the broader result/error names;
old SELECT type/error names remain aliases and SELECT-only methods retain their
family restriction. The raw fragment reader remains engine-owned and inert.

[Native saved crosstab reconstruction](NATIVE_SAVED_CROSSTABS.md) now uses the same
explicit saved-read APIs. SELECT-only entry points retain their family restriction;
named crosstabs with literal or qualified constant fixed headings now compose
through [metadata-only schemas](NAMED_CROSSTABS.md). Dynamic, parameter/policy-dependent
schemas now have [explicit execution-stage discovery](NAMED_SCHEMA_DISCOVERY.md)
for qualified relational sources and parameterless runtime domain targets.
Inline TRANSFORM remains unsupported. Stored TOP/owner options are preserved; positive crosstab TOP now executes, while
TOP 0/owner options remain refused rather than discarded to make a query run.
