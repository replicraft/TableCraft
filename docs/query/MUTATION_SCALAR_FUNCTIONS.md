# Deterministic scalar functions in UPDATE and DELETE

The bounded UPDATE and DELETE planners share one explicit deterministic scalar
binding owner. Both evaluate against original snapshot rows and finish through
one consuming backend preparation. This enables practical computed replacements
and predicates without importing native storage into the query crate, enabling
active content, or widening normal ACE save admission.

```sh
tablecraft query preview-update "UPDATE Contacts SET Name=Left(Trim(Name & 'X  '),20)" \
  --database contacts.accdb --output edited.accdb --variable

tablecraft query preview-delete 'DELETE FROM Items WHERE Abs(Id-2)=0' \
  --database offline.accdb --output deleted.accdb
```

Actual tables/fields must still meet the selected native engineering profile.
These examples create fresh experimental copies, not qualified native saves.
Ordinary `query update` shares expression admission while retaining its existing
single-column Int32/Jet 4 editing limits and ordinary ACE refusal.

## Closed function set

| Family | Admitted calls |
| --- | --- |
| Numeric/Boolean conversion | CBool, CByte, CInt, CLng, CLngLng, CSng, CDbl, CCur, CDec |
| Eager choice | IIf, Nz |
| Text length, slicing and spaces | Len, Left, Right, Mid, LTrim, RTrim, Trim |
| Numeric operations | Abs, Int, Fix, Round |

These are existing query function implementations and argument rules. Numeric
literals, existing literal/date/GUID forms, depth-zero original fields, unary and
binary operators, `IS NULL`, BETWEEN and IN lists retain their current behavior.
`IsNull(...)` is not part of this call set; the `IS NULL` operator remains available.
The text argument normalizer is reused, including valid optional substring
arguments. No ambient locale, clock, encoding or comparison policy is selected.
Text comparisons still require an explicit admitted host collation.

The function set excludes CStr, CDate and other date calls, case/encoding/formatting
APIs, Domain functions, Eval, Rnd, clocks, CurrentUser and user-defined or qualified
calls. Parameters, subqueries, aggregates, collections, correlated fields and
external/saved edit targets remain closed. Refusal occurs during binding, even
when WHERE is false or the snapshot is empty. This does not change the independent
[INSERT VALUES profile](../access/writes/SQL_INSERT_COPIES.md).

The binder uses a dedicated mutation-call capability enabled only for UPDATE
and DELETE. General retrieval function admission is not enabled for these plans.
The shared owner both binds the allowed calls and rejects accidental retrieval or
runtime nodes in the final bound expression. Other binder contexts retain their
existing capabilities.

## Original rows, eagerness and stored values

WHERE evaluates once per original row; every selected RHS evaluates against that
same unchanged row. Multi-assignment UPDATE retains simultaneous original values,
shared mask/tuple maintenance and one finish. DELETE collects exact original
snapshot handles before its one finish. There is no sequential candidate chaining.

All argument expressions remain eager. An unused IIf branch, an unnecessary Nz
replacement, or the right side of a false AND can still fail. A late divide,
conversion or overflow error produces no backend prefix and no published copy.
Calling a function does not introduce short-circuit behavior.

Assignment is a separate boundary. `ExistingInt32` still accepts only non-NULL
checked integral replacements; Currency, Decimal and Double results are not
silently narrowed. The opt-in `StoredScalar` profile adds nullable NULL and exact
other stored carriers. Use an admitted explicit conversion when its semantics are
intended, for example `CLng(Round(Score/2.0,0))`. Query-context `Nz(value)`
without a fallback replaces NULL with empty Text, not Empty or zero. That Text
still cannot be assigned to an Int32 field. Expression-only states cannot become
stored NULL/default markers.

Binding, original-row acquisition, evaluation, retained replacements and native
checks consume the supplied cumulative budget. Existing AST/name/depth ceilings,
fallible allocations and cancellation remain. The complete result must succeed
before backend finish and before host publication. Native constraints, historical
index rules, relationships and preservation still belong to the engine.

## Observed checks and limits

Three source-only gates passed before function admission was registered. They
reuse unchanged independent tuple images across both ACE headers (six expected
outputs), the actual DELETE owner (eight expected outputs), and the existing Text
source with separately frozen typed results. Text results are typed-row checks,
not complete raw-image qualification.

Seven new neutral function controls and all 42 adapter library functions passed
their first runs. They cover useful text changes, exact frozen tuple/DELETE
images, eager errors, original values, duplicate-target ordering, forbidden calls,
limits and final cancellation. The first broader query regression then stopped at
an older test that treated every DELETE call as unsupported. That exact Abs SQL
was migrated to a positive original-row selection with one backend finish; Rnd
and Domain refusals remain. The matching older empty-source UPDATE statements
were also retained as positive binding cases, with real-row NULL/wrong-carrier
refusals preserved. Predecessor sources and the first failed regression log are
retained. The corrected query regression passed 122 tests: 78 library functions
and 44 integration functions across eight targets. CLI checks passed 31 unit and
29 integration functions; all 42 adapter functions passed. Strict checks for five
packages and minimal checks for four libraries passed on their first runs.
The misleading new test name about omitted Nz states was then changed to describe
omitted-fallback carriers; this was a name-only correction after those checks.

These are local engineering checks. They do not qualify native Microsoft Access
function outcomes, data macros, normal ACE saves or crash recovery.
