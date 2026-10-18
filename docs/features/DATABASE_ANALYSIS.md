# Database analysis and developer workflows

Status: future requirements, 18 October 2026. Read-only catalog descriptors exist;
full object-body inventory, dependency graphs, semantic comparison and an application
test runner do not. Current metadata/parser support cannot prove a rename is safe.

VBA preservation and inert dependency detection remain required; VBA execution,
source authoring/refactoring, editor/debugger and runtime tracing are deferred.
VBA application-test scenarios below are retained future research, outside the
active roadmap. See [current VBA scope](../VBA_COMPATIBILITY.md).

## Inventory and refactoring

Build on read-only inventory with stable identities, known properties, preserved
source and explicit opaque objects. Dependency edges record origin/confidence:
declared bindings, recognized SQL/VBA references, dynamic strings and unresolved
external/opaque references are different evidence. Cover tables, queries, forms,
reports, controls, macros, modules and references. Static parsing cannot resolve
arbitrary runtime-generated SQL or VBA names.

Rename/migration previews show supported rewrites, affected objects, unresolved
dependencies and data loss. Block a claimed safe change when unresolved references
could break; any separately supported override needs an explicit label and
protected originals. Apply transactionally where supported; otherwise define
recovery before enabling the operation. Reparse/validate rewrites and reopen
copies in native Access. Illustrative affected-object counts are not measurements.

## Tools and acceptance

| Tool | Required result |
| --- | --- |
| Database Documenter | Structured metadata, definitions, relationships and automation with unknown/redacted sections marked |
| Database Compare | Semantic schema/object/source/property changes with stable matching and uncertainty; opaque byte/hash differences retained |
| Project-wide search | Names, SQL, VBA, macros, properties and text with locations; unindexed opaque content identified |
| Automated application tests | Explicit headless query/VBA/macro/form-event/report scenarios with controlled hosts/effects and oracle profiles |
| Schema migration preview | Dependency/data-loss analysis, supported rewrites and recovery plan before apply |
| Visual query debugger | Parameters, actual execution stages, measured costs/timings and bounded result probes once execution exists |
| Data validation dashboard | Qualified constraint/orphan/duplicate-candidate/reference checks, separate from authorized repair |
| Performance Analyzer | Measured query/form/report costs and evidence-based suggestions |
| Documentation/version control | Markdown/HTML, diagrams and supported semantic definitions, distinct from native regeneration |

Prioritize Documenter, Compare, search and application tests after useful native
inventory. Pair semantic differences with original hashes/format profiles;
normalization cannot hide unknown changes. Git-friendly definitions are not
automatically a lossless native serialization. Semantic comparison does not imply
a binary merge engine. Discovery must not execute startup content.

Analysis fits schema/model/query/lifecycle owners with design views and CLI
consumers as working slices emerge. Oracle tools stay in engineering. Bound and
authorize debug probes; inspecting action plans/effectful UDFs cannot execute them
accidentally. Acceptance needs real before/after copies, opaque/dynamic-reference
negative cases, failure/recovery evidence and native reopen in the
[corpus](../testing/ACCESS_APPLICATIONS.md). No such application baselines exist.

## References

Microsoft describes [Database Documenter/structural inspection](https://support.microsoft.com/en-us/access/learn-the-structure-of-an-access-database)
and [Object Dependencies](https://support.microsoft.com/en-au/access/use-the-object-dependencies-pane-to-see-how-objects-relate).
The proposed cross-object graph and safe-refactoring gates are TableCraft product
requirements beyond those references, subject to actual supported coverage.
