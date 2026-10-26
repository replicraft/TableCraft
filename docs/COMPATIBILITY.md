# Compatibility and support tiers

TableCraft 0.1.0 supplies a working CLI and bounded database services. It does
not yet provide complete Microsoft Access compatibility. The desktop UI and
general native editing remain in development. The [roadmap](ROADMAP.md) defines
the full acceptance scope; [implementation status](IMPLEMENTATION.md) records
current behavior and evidence.

## Planned priorities

| Tier | Formats | Direction |
| --- | --- | --- |
| 1 — Primary | ACE 16 | Full native compatibility is the long-term goal; new implementation and qualification focus here. |
| 2 — Secondary | Jet 4; ACE 12, 14 and 15 | Maintain working profiles and expand qualified support without displacing ACE 16 work. |
| 3 — Legacy | Jet 3 | Planned native reads and queries, starting from the existing header recognition. |
| Experimental | Jet 1.0/1.1 and Jet 2 | Optional bounded read-only feasibility research, beginning with original fixtures and reliable detection. |

These tiers describe priorities and targets, not completed compatibility,
release deadlines or permanent write exclusions. Writes require separate
preservation, validation, macro, transaction and recovery evidence for each
profile. Read-only scope is the starting boundary of the Jet 1/2 experiment,
not a claim that the wider compatibility goal has been met.

## Current native support

File contents select the reader. An extension, recognized header, engine release
number or independently decoded fixture does not qualify every stored feature.

| Family | Current status |
| --- | --- |
| ACE 16 — Access 2016 and later releases | Bounded 2016/2019-family metadata, local records and queries, qualified indexes/relationships and explicit advanced-field services. General ACE 16 table editing remains unavailable. |
| Jet 4 — Access 2000–2003 | Bounded metadata, local records, queries and qualified indexes/relationships; restricted fresh-copy fixed Int32 UPDATEs. |
| ACE 12 — Access 2007 | Bounded metadata, local records, queries and qualified indexes, with qualified complex links, multivalues and stored attachment/history reads. |
| ACE 14 — Access 2010 | Bounded ordinary table reads, including independently checked qualified Agile inputs. Advanced field/index variants need separate qualification. |
| ACE 15 — Access 2013 | Qualification follows the actual stored layout and features. It is not a separately qualified header family; do not infer full Access 2013 support from shared-layout reads. |
| Jet 3 — Access 95/97 | Header recognition only; legacy catalog, code-page, record and index reading remains planned. |
| Jet 1.0/1.1 — Access 1.0/1.1 | Detection and data extraction are not implemented or qualified. |
| Jet 2 — Access 2.0 | Detection and data extraction are not implemented or qualified. |

ACE engine releases do not map one-to-one to file-layout codes. In particular,
Access 2013 can use the ACE 2010-family layout; that does not establish a distinct
implemented ACE 15 header profile. Actual subversions, field types, indexes,
encryption and application features still require their own evidence. The
[native target matrix](NATIVE_READING.md#compatibility-targets) and
[ACE 16 profile](NATIVE_ACE16.md) provide the detailed boundaries.

Qualified encrypted reads and fresh encryption rewrite copies are separate from
general table mutation. Explicit cached calculation reads do not execute formulas;
recomputed views use a separate opt-in query service. Stored collections are not
VBA arrays, and lookup labels do not authorize stored RowSource execution.
See [encryption](NATIVE_ENCRYPTION.md), [advanced fields](features/ACCESS_ADVANCED_FIELDS.md),
[multivalue queries](MULTIVALUE_QUERIES.md) and [native writes](NATIVE_WRITES.md).

## Runtime, compiled and packaged files

| Extension | Implemented read route and limits |
| --- | --- |
| `.mdb`, `.accdb` | The qualified Jet/ACE profile selected from file contents. |
| `.accdr` | The underlying native profile; the suffix does not add runtime execution. |
| `.accde`, `.mde` | The underlying native profile for local records. Filename routing does not qualify an Access-produced compiled application's full behavior. |
| `.accdt` | Bounded OPC/template inventory and qualified stored schema/data reads, including supported inline and List Definition declarations. Wider template/application behavior remains unfinished. |
| `.accdc` | Bounded single-database CAB extraction feeding the qualified contained database reader. Signatures remain unverified; no installation or trust decision is reproduced. |

Stored package names, links, paths and external relationships do not authorize
filesystem writes, target opens or active-content execution. Larger qualified
packages can use explicitly bounded temporary spooling; defaults and native,
query and output limits still apply. See the
[distribution contract](features/ACCESS_DISTRIBUTION.md),
[templates](ACCESS_TEMPLATES.md) and [stored template data](ACCESS_TEMPLATE_DATA.md).

## SQLite is a separate backend

Existing SQLite files can supply bounded ordinary scalar tables through
`query run --sqlite PATH`. Access SQL is evaluated by TableCraft, not delegated
to SQLite. The adapter is read-only and does not create files, edit schemas,
load extensions or implement complete Access-to-SQLite conversion. Its type,
object and snapshot limits are documented in [SQLITE.md](SQLITE.md).

## What the evidence establishes

Automated tests, unchanged original fixtures, independent-reader comparisons and
engineering checks establish their recorded scopes. They do not replace native
Microsoft Access SQL, save/reopen, application, concurrency or platform
qualification. Deferred Windows/VBA execution and integration remain explicit
compatibility gaps rather than completed features. Current CLI commands do not
implement Access forms/reports, a VBA runtime or a GUI.

Reads and queries remain the immediate native backend priority before wider
writes. The [read/query acceptance gates](READ_QUERY_ACCEPTANCE.md),
[native checks](NATIVE_CHECKS.md), [fixture provenance](FIXTURES.md) and
[roadmap](ROADMAP.md) distinguish working services from remaining implementation
and qualification. Use the [CLI guide](CLI.md) for current command examples.
