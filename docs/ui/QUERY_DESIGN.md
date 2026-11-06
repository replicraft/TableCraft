# Visual query design

The scratch SQL object has a bounded visual SELECT designer. It shares the same
object tab and SQL buffer with SQL View; saved-query definitions are not changed.
The headless `tablecraft-builders::query` module owns the design model, expression
parsing, criteria composition, SQL import and canonical SQL generation. The
shared UI renders that model directly. Runtime services qualify native table
metadata; the desktop host dispatches those services and the existing query
executor.

## Working flow

The Ribbon View dropdown selects SQL, Design and cached Datasheet results. It is
available from Home as well as the contextual Query Design tab. Selecting a
cached result does not rerun SQL. The contextual tab remains available across
these views; Run appears once in each layout. Add Tables belongs to the Design
Ribbon rather than a second toolbar above the canvas.

Switching an empty SQL object to Design opens Add Tables. Selecting a table is
inert; Add explicitly requests its field metadata. Double-clicking an upper
source field adds a typed projection to the lower QBE grid. A source's `*` adds
its fields without inventing expanded field declarations.

The grid exposes Field, Table, Sort, Show, Criteria and OR rows. Field expressions
and criteria are query syntax, independent of regional display settings. Entries
on one criteria row combine with AND; populated rows combine with OR. Hiding a
projection does not remove its criteria. Output aliases and column removal are
available from the field cell's context menu. Explicit source-field joins retain
their two source identities, join kind and field names.

SQL View explicitly builds the current draft. Run explicitly builds and then
executes the returned canonical SQL through the existing read-only query service.
Run displays Datasheet results for the scratch object. A pending or failed run
retains previous results with a qualification message; it never treats those
rows as the new draft's completed result.

Switching existing SQL to Design explicitly imports it. Unsupported or
nonrepresentable clauses refuse with a visible diagnosis while preserving the
original SQL, prior draft and previous results. Import is not permission to
execute the query. Inspecting a saved query remains inert and separate.

## Source and resource rules

Each operation captures the database revision, exact scratch object instance,
UI publication and draft revision. Canonical execution additionally captures the
exact generated SQL and its own request ID. Cancelled or superseded replies
cannot populate a reopened scratch object. A successful result must be a complete
zero-offset query result with matching row widths and no native row identities.
There are no native edit rights on design results.

The builder profile permits eight sources, seven explicit joins, 32 output-grid
columns, eight criteria rows and 64 KiB of combined editable UTF-8 text. Metadata
has separate owner-defined limits. The UI rejects an oversized insertion whole
before text layout; it does not keep a truncated prefix. Runtime still validates
the complete draft and fresh metadata on Build and Import.

Drafts and view choice follow the scratch object across object switches. Workspace
restoration retains only source-free draft choices and SQL. It discards metadata,
requests, completed result receipts and rows. A previously displayed Results view
returns to Design after restoration and requires fresh service qualification.
Upper cards may show that field metadata is not loaded until an explicit Build
or Add Tables request obtains fresh descriptors. Schema loading is not claimed
to happen automatically during restoration.

## Current limits and visual evidence

This is a SELECT designer, not a saved-query editor or a schema designer. It does
not provide action-query authoring, grouping, totals, dynamic pivot design or
arbitrary SQL round trips. Unsupported shapes remain available in SQL View
through the existing explicit query profile. Native query admission may reject a
design that can be represented syntactically.

The upper cards, divider, QBE row organization and Add Tables list follow the
actual Microsoft images recorded in [visual references](VISUAL_REFERENCES.md).
Source cards have a bounded field-list viewport with their own vertical scroll
on a gray canvas. The current Field cells retain qualified expressions rather
than Access's shorter local field labels; the Table row still identifies the
source separately.
The current join workflow uses field context menus and explicit join-kind
controls; native drag-and-drop and the Join Properties dialog are not qualified.
The Add Tables surface exposes the implemented native-table source profile,
without nonfunctional Queries/Both tabs. Field aliases use a context editor.
These distinctions remain visual and interaction approximations, not verified
Access behavior.

The `query-design`, `query-design-tables` and `query-design-error` preview scenes
use authored synthetic metadata and drafts. Interactive mock Build/Run refuses
with an explicit synthetic notice. It does not scan data, calculate visible-page
results or invoke a native database. No native Windows or Access comparison has
been performed for this slice.
