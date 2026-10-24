# Stored text-call validation audits

Reviewed 24 October 2026. The [original producer](ValidationTextCallFixture.java),
[read-only oracle](ValidationTextCallOracle.java) and
[independent runner](compare_validation_text_calls.py) exercise bounded stored
field/table validation rules containing Len and explicit binary StrComp. This is
read-audit evidence under the current engineering expression profile, not Access
execution or native save qualification.

## Primary contracts and explicit scope

Microsoft's [validation-rule guide](https://support.microsoft.com/en-us/access/restrict-data-input-by-using-validation-rules)
provides the field-rule example `Len([StringFieldName])<100`. Its reference table
also demonstrates StrComp with explicit comparison mode 0 in an uppercase rule.
That broader example does not admit UCase here: the implemented read-audit subset
contains only Len(Text/NULL) and StrComp(Text/NULL, Text/NULL, 0), with the owning
rule compiler's existing pure Text-expression and field-scope checks.

The [DAO Field.ValidationRule contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-validationrule-property-dao)
restricts a field rule to its own field and excludes UDFs, aggregate functions and
queries. The corpus checks a field's own named references and refusal of an
otherwise valid Text call that references another field. Table rules can use the
explicit same-table fields. These documents support rule placement and call
availability; the observed independent behavior below does not qualify every
Access function, promotion rule or locale.

## Separate reference checks and inert inputs

The producer defaults to expression evaluation disabled. It stores original
ValidationRule properties and ordinary data, including deliberately invalid
historical states whose rules are attached after insertion. No stored function,
unknown call, clock, locale-dependent comparison or imported content executes
while generating these main corpus files.

A separate explicit `--reference-check` mode creates fresh pure-rule probe files.
Only the original supplied Len/StrComp rules evaluate in this mode. Assertions
check accepted and rejected insert attempts against Java String UTF-16 length
and binary ordering. Direct NULL outcomes produce the distinct reference error
`Condition evaluated to Null`; the code distinguishes it from a false-rule
rejection. Explicit IS NULL guards admit intended NULL inputs. The read-only
oracle then confirms the separate reference probes and their inert main-corpus
equivalents retain identical getters and rule declarations.

The oracle always disables evaluation, opens originals read-only and emits exact
stored field/table properties and tagged ordinary getters. The runner models Len
by counting unsigned UTF-16 units and models binary ordering by lexicographic
comparison of unsigned unit vectors. It does not compare UTF-16-LE bytes or Python
Unicode code points. U+00FF/U+0100 and BMP/astral surrogate discriminators separate
these possible mistakes. Combining characters, empty text, embedded NUL, control
characters, case, punctuation and trailing spaces remain explicit inputs.

## Read-audit controls

Each profile contains 51 tables and 103 stored rows. Ten rules are Checked,
including empty admitted rules and guarded NULL outcomes. Thirty-three rules
remain Unsupported: unresolved NULL outcomes and excluded call names, omitted or
locale comparison modes, numeric arguments, qualified references and field-scope
violations. Unsupported syntax stays Unsupported in empty tables.

Eight late-FALSE rules per profile must reject the audit without stdout. Separate
100-ASCII, 50-astral and 98-ASCII-plus-NUL-plus-character values catch distinct
length mistakes; the latter two each contain exactly 100 UTF-16 units. Binary
case failures remain separate from binary order controls. Ordinary queries still
read every table, including invalid/unsupported rule states, without executing
those stored properties or silently asserting their consistency.

The [accepted report](results/validation-text-calls-2026-10-24.json) records 204
passing controls and 618 compared getter cells across ACE 2016/2019. All 19 pinned
implementation, producer/oracle/runner, helper, original input, reference JAR and
immutable executable hashes remain unchanged. The CLI SHA-256 is
`36613d7cac56199e04337385e0fbdcebdfe425dc6fed592e463363f2710d3eca`.
No native Access execution, Windows comparison or broader function admission is
inferred. [Fixture provenance](../FIXTURES.md#original-text-call-validation-inputs)
records original hashes and excluded probes.

```sh
python3 docs/testing/compare_validation_text_calls.py \
  --cli .cache/tablecraft-bin/validation-functions-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/validation-text-calls-qualified-20261024 \
  --report /tmp/validation-text-calls.json
```

## Existing ACE and Jet 4 regression replay

The existing original `ValidationFixture.java` recipe was also regenerated as
fresh ACE 2016 and Jet 4 files. The updated `compare_validation.py` passes all
12 rule-coverage checks against the same immutable executable. Populated and
empty Len rules are now Checked; unresolved NULL outcomes remain Unsupported.
Both input hashes remain unchanged. This regression replay is separate from the
204-control corpus above and does not expand its profile/count claims.
