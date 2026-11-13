# Visual query design

## Saving and closing Design

Saved Design belongs to its actual saved object, including while parked. Build
and Run use that editor's authored SQL rather than the global scratch buffer or
old stored definition. Run of the stored object cannot bypass the current Design
owner; executing that Design goes through checked Build and the existing issued
parameter flow. SQL View instead owns its current raw SQL, even if an older typed
model or historical Design datasheet remains. Such cached rows cannot select that
old model for saving newer SQL.

Save builds first and retains Design mode. Scratch Save then requests a name;
ordinary saved Design Save uses the existing qualified expression-edit review.
Save As creates from current authored SQL. Successful scratch creation rebinds
the same tab to the actual saved identity; saved Save As keeps the original dirty
source and opens a distinct created tab. Build alone never marks a changed query
natively saved; accepted full-witness publication resolves only its affected owner.
Exact typed equality, not request revision equality, detects Design edits. Unchanged
Build/Run and explicit Field Cancel back to baseline remain clean; Run of changed
input remains dirty. Already-dirty saved SQL import cannot become the stored baseline.

Changed parsed Design and nonempty scratch SQL close through exact
Save/Discard/Cancel. Raw SQL Close-Save names the actual text without building a
hidden Design; Design Close-Save first builds its current model. Explicit Discard
drops the exact owner without treating an unbuilt model as saved. Stale choices
cannot discard newer edits. Cancelled naming, unsupported admission or a failed
save cannot arm a later unrelated Save to close the tab or resume deferred Open/Exit.
See [saved-query editing](../application/SAVED_QUERY_EDITING.md) for current native
source/serializer restrictions and the source-pinned GUI lifecycle record.
Native saved-SQL review diagnostics are visible in Design and Results as well as
SQL View, without changing that writer's alias/model eligibility. Created queries
obtain ordinary edit eligibility from real Runtime inspection. Its canonical stored
comparison text never replaces authored SQL, advances its epoch or changes Design.

Scratch and saved-query objects have a bounded visual SELECT designer. Each
retains its own authoring SQL, Design draft and revision; changing views or
building SQL does not itself change a stored definition.
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

The grid exposes Field, Table, Sort, Show, Criteria and OR rows, with a Total row
when query-design Totals is enabled. Field expressions
and criteria are query syntax, independent of regional display settings. Entries
on one criteria row combine with AND; populated rows combine with OR. Hiding a
projection does not remove its criteria. The Field row displays an output alias
as `Alias: operand`; names requiring quoting use the SQL owner's bracketed spelling.
The same cell edits that prefix and operand. Column removal and the separate alias
control remain in its context menu; the separate alias control is disabled while
a raw Field edit owns the input. Explicit source-field joins retain
their two source identities, join kind and field names.

SQL View explicitly builds the current draft. Run explicitly builds and then
executes the returned canonical SQL through the existing read-only query service.
Run displays Datasheet results for the owning scratch or saved Design object. A pending or failed run
retains previous results with a qualification message; it never treats those
rows as the new draft's completed result.

Submitted build drafts use immutable `Arc` ownership across the action, pending
UI witness and runtime command. The editable session remains separate, so later
changes cannot mutate submitted work. Full content/revision matching is retained;
every distinct model allocation retains its complete logical text/vector/control
charge. Pointer-identical baseline/submitted/pending snapshots are charged once;
independently mutable editor storage remains separate. A complete incoming import
model, metadata, snapshots and SQL is admitted together before publication. The
[ownership audit](../development/CLONE_AUDIT.md) records pointer-sharing and
retention checks. QBE text-edit candidates borrow with `Cow` until actual input;
rejection preserves the original text and the existing input budgets.

The current Results view explicitly owns either Design or SQL authoring state.
An accepted Design Build/Run establishes that owner before result rows arrive;
selecting a historical Datasheet retains the current editor owner instead of
reviving the model that produced those rows. Result provenance and validated
submitted SQL still qualify displayed rows separately. Save, close, parameters
and Run follow current authoring ownership, with unchanged freshness checks.
See the [SQL workspace contract](SQL_WORKSPACE.md) for retention and result labels.

Switching existing SQL to Design explicitly imports it. Unsupported or
nonrepresentable clauses refuse with a visible diagnosis while preserving the
original SQL, prior draft and previous results. Import is not permission to
execute the query. Inspecting a saved query remains inert and separate.

## Grouped design and declarations

The contextual Query Design Totals command changes the draft's grouping mode; it
is separate from the [datasheet Totals footer](DATASHEET_TOTALS.md). The typed
Total choices are Group By, Sum, Avg, Min, Max, Count, First, Last, Expression and
Where. Group By retains its key even when Show is off. Aggregate choices wrap the
captured field or scalar operand once; Count(field) excludes NULL values, while
Count(*) is entered as an explicit Expression. Expression also represents
complete grouped expressions using the existing query profile. Selecting a role
does not promise that every input type or expression will bind successfully.

Plain native Sum, Min and Max fields without explicit aliases receive the
English `SumOf<Field>`, `MinOf<Field>` and `MaxOf<Field>` headings described by
Microsoft's [sum-query guide](https://support.microsoft.com/en-us/access/sum-data-by-using-a-query)
and [top/bottom-values guide](https://support.microsoft.com/en-us/access/find-the-records-with-the-top-or-bottom-values-in-a-group-or-field).
Explicit aliases and Expression-role text keep their supplied names. Generated
heading collisions refuse instead of inventing suffixes. Defaults for the other
roles retain current query-owner naming; their native default-heading parity,
localized prefixes and truncation/disambiguation rules remain unqualified. This
does not impose an explicit-alias requirement on those roles. Import still
refuses if adding a default would rename an existing query's implicit output.

Where is available only after the user hides that column and removes its sort.
It filters input records in WHERE and is never added to output or grouping.
Other Total roles place criteria in HAVING, against the actual group key or
aggregate expression. Criteria such as `>100` on Sum(Amount) compare that
aggregate directly; an output alias is not substituted as a source field.
Sorting likewise retains the captured key or aggregate operand.

AND/OR grid composition is preserved within each clause. A single populated row
may contain both Where and HAVING criteria. Alternative rows spanning both
phases are currently refused because factoring those alternatives into separate
WHERE and HAVING clauses can change which records are grouped. This is an editor
qualification boundary, not an Access SQL restriction; SQL View retains the
original statement. Turning Totals off is also conservative: nondefault roles or
explicit aggregate expressions must be edited deliberately first. An unavailable
toggle does not reset roles, clear text, rebuild SQL or execute a query. Native
Access toggle-off restoration has not been qualified.

Imported PARAMETERS declarations remain owned typed syntax in the draft, in their
written order. Fields resolve against actual source metadata before an unqualified
declared parameter; a qualified field cannot become a parameter. SQL View remains
the place to edit the declaration header. The existing explicit Run acquisition
collects values for declared inputs, with cancellation and revision checks. A
parameter declaration grants no execution or native editing capability.

The builder exports `render_parameters` and `parse_parameters` for consumers such
as MCP. They use the SQL owner's Name/Type representation and strict parsing;
extra statements, malformed declaration syntax and duplicate names are refused. Hosts do not
maintain another parameter type grammar.

Grouped import retains projection order, hidden keys, declarations, aliases,
WHERE, GROUP BY, HAVING and sorting through the canonical AST round-trip check.
Only transparent table parentheses and recognized aggregate function-name case
are normalized; field and alias names are not rewritten. Unsupported joins,
clause combinations, alias references or source expansions remain explicit import
errors. Import does not drop clauses to make a statement fit the grid. The
builder validates metadata and produces syntax without scanning user records;
the existing query binder/executor owns grouping, type, arity, eager errors,
collation and expression-provider semantics.

The [QBE checkpoint](../../tests/results/queries/qbe-totals-workflows-2026-11-12.json)
records executed owner/Runtime/Desktop/MCP checks, translation validation and
directly viewed offscreen frames. This is local qualification; code or a
synthetic preview is not native Access interaction or Windows parity evidence.

## Retained Field edits

Incomplete Field text remains authored data even when it cannot yet replace the
parsed column. Blur, editing another column and switching objects retain the exact
text. Source refresh retains that text and its explicit-Cancel baseline while
removing focus hints, metadata and execution rights. A current typed Field keeps
its logical source identity and is rechecked by the builder; the Cancel baseline
cannot supply a source binding.

Each raw edit advances the draft revision. Build, SQL publication, import and Run
refuse unresolved text; a previously queued action or result cannot execute the
older parsed model behind a newer input buffer. Hard parser, allocation and input
limits reject the complete candidate rather than retaining a truncated prefix.

Escape in the Field editor, or **Cancel field edit** in its context menu, restores
that field's original operand and alias. Other columns, criteria and roles stay
as edited. A blocked close reveals the affected Field so this cancellation remains
reachable after blur. While an edit is unresolved, closing that SQL tab or taking
a destructive document/window transition requires finishing or explicitly
cancelling it. Parked edits also block application restart. This is a temporary
editor restriction alongside the exact Design close workflow above. Native
Access close interaction has not been observed.

The [field-authoring checkpoint](../../tests/results/queries/qbe-field-authoring-workflows-2026-11-13.json)
records real Field typing, parser-limit refusal, focus/Cancel ownership, stale
action rejection, parked input and acknowledged native-save/fresh-reopen
preservation. It also pins four directly viewed English/German light/dark
normal-host frames. These local observations are separate from native
Windows/Access behavior; that earlier checkpoint does not qualify the later
Save/Save As integration described above.

## Keyboard flow

F6 reaches the source lists and lower grid as separate panes, followed by the
existing Navigation and Ribbon stops; Shift+F6 reverses that cycle. Only enabled
controls rendered for the current object and view are focus targets. An empty
grid can be entered through Add expression. Modal dialogs and child popups retain
their input ownership.

Each source list has one Tab stop. Up/Down and Home/End move its current field and
reveal it inside the card. Space toggles that exact typed field's output; `+`
shows it or adds it if absent, and `-` hides a visible matching projection. These
operations retain aliases, criteria and unrelated projections. Duplicate matches
are handled one at a time rather than bulk-deleted. Enter does not add a query
field; pointer double-click retains its existing append behavior.

The Field editor's Shift+F10 opens its alias/remove context menu, including explicit
Cancel while an edit is retained. Escape from a keyboard-opened menu returns focus
to that editor; Escape in the editor cancels its retained Field edit. Tab and Shift+Tab use
the actual input controls; text-editing arrows remain text-editing keys.

A focused source field also accepts Shift+F10 for its existing Join to menu.
Keyboard activation enters the first available source/field item; Tab can choose
another explicit target before Enter creates the join. Escape from the root menu
returns to the source field. These menus share their current-owner popup helper;
pointer opening retains pointer anchoring. No join is inferred from names or
relationships, and opening the menu never runs a query.

Microsoft's [Access keyboard contract](https://support.microsoft.com/en-us/accessibility/access/keyboard-shortcuts-for-access)
documents F6 between Query Design panes, diagram arrows/output keys and the
context-menu shortcut. Home/End and continuing the cycle through Navigation and
Ribbon are current list/workspace adaptations. Native column-selection,
multiselection, overtype and complete cell-navigation parity remain unfinished.
No Windows/Access interaction comparison has qualified this implementation.

## Source and resource rules

Each operation captures the database revision, actual scratch or saved object instance,
UI publication and draft revision. Canonical execution additionally captures the
exact generated SQL and its own request ID. Cancelled or superseded replies
cannot populate another or reopened object. A successful result must be a complete
zero-offset query result with matching row widths and no native row identities.
There are no native edit rights on design results.

The builder profile permits eight sources, seven explicit joins, 32 output-grid
columns, eight criteria rows and 64 KiB of combined editable UTF-8 text. Metadata
has separate owner-defined limits. The UI rejects an oversized insertion whole
before text layout; it does not keep a truncated prefix. A retained Field edit
reserves the greatest of its raw text, Cancel baseline and current parsed payload
in that editable allowance, so another edit cannot consume the bytes needed for
Cancel. Workspace retained-memory accounting separately charges the actual raw,
baseline and model storage, including shared immutable owner handles. Runtime still validates
the complete draft and fresh metadata on Build and Import.

Drafts and view choice follow their scratch or saved object across object switches.
Workspace restoration retains only source-free draft choices and SQL. It discards
metadata, requests, completed result receipts and rows. Restored Design-owned
Results returns to Design; SQL-owned Results returns to SQL. Both require fresh
service qualification before execution.
Upper cards may show that field metadata is not loaded until an explicit Build
or Add Tables request obtains fresh descriptors. Schema loading is not claimed
to happen automatically during restoration.

## Current limits and visual evidence

This is a SELECT designer, not a saved-query editor or a schema designer. It does
not provide action-query authoring, dynamic pivot design or arbitrary SQL round
trips. Grouping follows the bounded Total-role and clause rules above.
Unsupported shapes remain available in SQL View
through the existing explicit query profile. Native query admission may reject a
design that can be represented syntactically.

The upper cards, divider, QBE row organization and Add Tables list follow the
actual Microsoft images recorded in [visual references](VISUAL_REFERENCES.md).
Source cards have a bounded field-list viewport with their own vertical scroll
on a gray canvas. Bound Field cells show their local names, with the source alias
or table in the separate Table row. Focus and unchanged replacement retain the
typed source identity. Known local-name replacements and their incremental
prefixes keep that source; incomplete names still require checked Build admission.
Other text uses the existing expression path and clears the Table association.
Correcting such text does not guess a source for an ambiguous unqualified name.
This is a bounded editor, not general native Access field-editor parity.
The current join workflow uses field context menus and explicit join-kind
controls; native drag-and-drop and the Join Properties dialog are not qualified.
Join controls show source-qualified field endpoints and expose distinct localized
names for their kind and removal controls. Identical field names in different
sources therefore remain distinguishable; these labels are presentation text,
never SQL input. Missing source metadata remains visible rather than renamed.
The Add Tables surface exposes the implemented native-table source profile;
Queries/Both tabs remain visible but disabled. Field aliases use the visible prefix
or the separate context control when no retained Field input owns the edit.
These distinctions remain visual and interaction approximations, not verified
Access behavior.

The `query-design`, `query-design-tables`, `query-design-totals` and
`query-design-error` preview scenes
use authored synthetic metadata and drafts. Interactive mock Build/Run refuses
with an explicit synthetic notice. It does not scan data, calculate visible-page
results or invoke a native database. No native Windows or Access comparison has
been performed for this slice.

The retained query-design captures establish the neighboring ruled grid and
source layout, but do not show the Total row or its complete dropdown. Its row
placement and popup presentation are adaptations pending a corresponding Access
capture. Datasheet Totals images do not qualify that separate query-design
surface. No new native-window, dark-mode or Windows comparison is claimed here.
