# SQL INSERT engineering copies

`tablecraft query preview-insert` prepares one explicit local `INSERT ... VALUES`
row and publishes a fresh experimental copy through the existing ACE engineering
service. It does not enable SQL INSERT in normal writable sessions, qualify data
macros, execute stored code, or establish Microsoft Access save compatibility.

```sh
tablecraft query preview-insert \
  "INSERT INTO Contacts (ID, Name, Score) VALUES (42, 'Ada', NULL)" \
  --database offline.accdb --output inserted.accdb
```

The target must be one local table. Every ordinary field must be supplied exactly
once. An explicit field list may reorder the complete set; without a field list,
manual VALUES follow the complete native declaration order. Missing, repeated,
unknown and external fields refuse before a backend candidate is prepared.
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
requires a complete explicit companion field list, excluding exactly the actual
generated primary. Positional VALUES never silently omit a generated field.
Supplying the generated field in this mode, omitting another companion, or
omitting the field list refuses. The current VALUES profile also refuses an
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
this is not advertised as a metadata-only generation inspector. Qualified text
default declarations remain inert because every ordinary value is supplied.
No default expression is evaluated or substituted for explicit NULL. Required,
AllowZeroLength, validation, NewValues, index, relationship and active-content
gates remain authoritative.

## Source and publication boundaries

The CLI checks closed syntax and the incrementing field-list requirement before
opening the source. One immutable image feeds one backend candidate. The generic
`InsertableTable` contract contains metadata and a consuming complete-row finish;
it has no concrete engine, source-opening, scan or publication capability.

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
