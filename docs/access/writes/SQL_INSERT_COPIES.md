# SQL INSERT engineering copies

`tablecraft query preview-insert` prepares one local `INSERT ... VALUES`
row and publishes a fresh experimental copy through the existing ACE engineering
service. It does not enable SQL INSERT in normal writable sessions, qualify data
macros, execute stored code, or establish Microsoft Access save compatibility.

```sh
tablecraft query preview-insert \
  "INSERT INTO Contacts (ID, Name, Score) VALUES (42, 'Ada', NULL)" \
  --database offline.accdb --output inserted.accdb
```

The target must be one local table. An explicit field list may reorder or omit
ordinary fields; explicit values remain distinct from omissions. Without a field
list, manual VALUES must supply the complete native declaration order. Repeated,
unknown and external fields refuse before a backend candidate is prepared.
The existing complete-row library planner/service still refuses named subsets;
the CLI selects the separate presence-bearing planner described below.
INSERT SELECT, parameters, subqueries, identity reads and user-defined functions
are outside this first profile.

Literal values, bounded pure operators and the existing deterministic carrier
constructors `CBool`, `CByte`, `CInt`, `CLng`, `CCur`, `CDec`, `CSng` and `CDbl`
are admitted. Other calls, including ambient clock, domain and dynamic Eval calls,
refuse before a database is opened. Expressions evaluate in written VALUES order
before one backend finish. A later expression error returns no partial candidate.
`NULL` remains distinct from the text `'null'` and the present empty text `''`.

The planner shares the existing stored-scalar assignment law with SQL UPDATE:
Int32 accepts checked integral values; other non-NULL carriers must match the
actual declared type and constraints exactly. There is no implicit Text, decimal
scale or floating-point conversion. The current SQL expression profile cannot
construct every non-NULL carrier available through the
[typed INSERT input](ENGINEERING_COPIES.md#complete-row-insert), such as arbitrary
Binary or Extended Date/Time payloads.

## Explicit incrementing policy

```sh
tablecraft query preview-insert \
  "INSERT INTO Contacts (Name, Score) VALUES ('Ada', NULL)" \
  --database offline.accdb --output inserted.accdb --incrementing
```

`--incrementing` explicitly selects increment-by-one engineering behavior. It
requires an explicit companion field list, excluding exactly the actual
generated primary. Ordinary companions may be omitted under the same needed-only
default policy. Positional VALUES never silently omit a generated field.
Supplying the generated field in this mode or omitting the field list refuses. The current VALUES profile also refuses an
empty supplied-column schema, including generated-only tables.

Generated-field identification remains inside ACE. The engine returns its actual
declaration position/name and a policy pinned to the captured LAST counter; the
adapter does not infer a field from its name, ordinal or raw flags. The existing
INSERT preview independently revalidates that policy. This neither certifies
stored Increment/Random mode nor computes MAX(live ID), reseeds a table or grants
a normal save. The existing exact-counter API and
[`write preview-insert --incrementing-last`](ENGINEERING_COPIES.md#complete-row-insert)
remain available separately.

Ordinary column metadata is built without a row snapshot. Explicit generation
qualification can perform the engine's complete source checks before binding;
this is not advertised as a metadata-only generation inspector. Defaults remain
inert during metadata inspection. No default expression is substituted for
explicit NULL. Required,
AllowZeroLength, validation, NewValues, index, relationship and active-content
gates remain authoritative.

## Named omissions and needed defaults

```sh
tablecraft query preview-insert \
  "INSERT INTO Contacts (ID, Name) VALUES (42, 'Ada')" \
  --database offline.accdb --output defaulted.accdb
```

The [presence-bearing planner](../../query/INSERT_OMISSIONS.md) carries a full-width
row of `Supplied(Value)` or `Omitted` through one consuming adapter finish.
`Supplied(NULL)` stays an explicit NULL. Missing fields without a default resolve
to NULL, then ordinary Required, carrier, validation, unique/index and
relationship rules remain authoritative. Boolean absence is not silently False.
Only the engine interprets a needed original default; the query/adapter do not
parse native properties or expand values and discard the original omission intent.

The initial engine policy admits closed literal NULL, Boolean and Text values,
and exact signed numeric constants for qualified integer, Currency and Decimal
fields. Decimal storage precision/scale and exact rescaling apply without a
binary-float detour or rounding. Calls, names, runtime/clock defaults such as
`Date()`, unsupported carrier forms and malformed needed expressions refuse.
This does not execute arbitrary DefaultValue SQL or certify native Access timing.
Broader default expressions remain future compatibility work.

An unsupported or malformed default on a supplied field remains inert, subject
to the existing declaration checks. Entirely supplied rows use the unchanged
complete insertion service. Partial rows retain original presence through the
engine's independent audit, including the explicit generated companion schema;
there is one captured image and one candidate, with no preview retry or chaining.

## Source and publication boundaries

The CLI checks closed syntax and the incrementing field-list requirement before
opening the source. One immutable image feeds one backend candidate. The generic
`InsertableTable` contract retains its metadata and complete-row finish. The
separate `OmittingInsertableTable` capability preserves presence without adding
SQL/native metadata, source-opening, scan or publication capabilities to DB.

The host retains its read-only source guard through fresh-output publication.
Encrypted and deployment-package inputs refuse; providing a password does not
permit plaintext rewriting. Output uses the
[common no-clobber publisher](ENGINEERING_COPIES.md#bounds-and-publication), with
one output-byte charge, an owned temporary, sync and fresh-name publication.
Source aliases and existing destinations are never replaced. Generated facts
are reported only after publication; a later stdout failure can leave the
completed copy in place.

Defaults are 256 MiB cumulative bytes, 100,000 rows and 100,000,000 steps, adjustable
through the work-limit flags. Parser, evaluator and engine structural limits
remain separate. The first VALUES profile permits at most 256 supplied columns,
uses bounded expression depth/work, and refuses unsupported shapes without
fallback. These limits do not define the eventual ACE compatibility target.

## Verification status

The independent literal source/whole-expected gate passes before any new SQL
candidate calls: four cases cover both ACE16 layouts with manual and nonfirst
column generation. Source records, counters, maps and the retained four-byte
index prefix are checked independently of the producer. The first source build
reported unused functions in two reused fixture modules; narrow allowances on
those imports changed no fixture or expected bytes.

All four generated-binding functions pass, including its independent source gate,
actual nonfirst generated position/name/LAST, unknown NewValues refusal, paid
field-name allocation and measured terminal cancellation. All nine neutral
planner controls pass. The actual CLI target passes all three functions, including
source qualification, four byte-exact manual/generated candidate comparisons and
closed-shape refusals before database open. The 78 query library tests and 16
existing query integration tests also pass. These are first runtime passes;
no source or expected-image correction followed an API result.

Broader verification passes: all 296 INSERT library functions, 36 Access
engineering controls, 31 CLI unit functions and 23 CLI integration functions.
Strict lint passes across all five affected packages, and minimal builds pass
for all four libraries. No native Microsoft Access execution or save comparison
is claimed.

## Omission frontend validation

The first frontend source gate qualified eight complete source/expected pairs
before any new frontend API: both headers with constant Score40 omission,
nullable unindexed Id omission, supplied Score under an inert `Date()` default,
and an actual generated Score at native column one with omitted Other default99.
The C3 generated derivative starts from the existing genuine C2 literal owner,
retains its primary INDEX/maps/counters and adds explicit three-field rows and
a scoped literal property. A static review corrected its maximum native slot
count from two to three before the first source run; the predecessor constructor
and freeze remain preserved. No API result changed its expected bytes.

All four frontend functions passed on their first run: eight direct Access SQL
service results and eight CLI fresh copies match the complete frozen images,
including `[Id=4, Score=31, Other=99]` under explicit `--incrementing`. Six CLI
needed-unknown-default, explicit-NULL Required and duplicate-key cases produce no
output. Original source files remain byte-identical. The shared current batch
passed 498 engine mutation functions, 72 shared native functions across ten
targets, 51 Access library functions and 84 query library functions. CLI checks
passed 31 unit and 33 integration functions: six DELETE, two historical, nine
INSERT, twelve UPDATE and four normal Jet 4 controls. Strict checks across five
packages and current ACE/Access minimal checks passed. The earlier totals above
belong to the complete-row checkpoint; 498 is a mutation subset, not a complete
ACE library count. After those checks only the CLI command-summary help wording
was clarified to mention named omissions. No Microsoft Access execution or save
comparison is claimed.
