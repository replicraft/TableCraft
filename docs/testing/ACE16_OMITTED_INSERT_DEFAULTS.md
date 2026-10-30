# Omitted fields in ACE 16 INSERT previews

The omission-aware engineering INSERT APIs preserve the difference between a
supplied value and an omitted ordinary field. Supplied NULL remains NULL. An
omitted field uses its qualified source DefaultValue, or NULL when no default
exists. Unknown needed defaults refuse; they are never replaced with an invented
NULL. Fully supplied values keep existing defaults inert.

Microsoft documents default-or-NULL insertion for missing columns. Its default
contract applies when no value is specified for that column. Keeping explicit
NULL separate follows those contracts; this is not a newly observed native Access
execution. [INSERT INTO](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql),
[field defaults](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/modify-a-table-s-design-using-access-sql)

## Engine contract

`OmittedRowInsert` owns a full-width vector of `InsertFieldValue::Supplied(Value)`
or `InsertFieldValue::Omitted`, in the selected supplied-schema order. The ordinary
`preview_ace16_insert_rows_with_omissions` method uses native declaration order.
The explicit incrementing counterpart,
`preview_ace16_incrementing_insert_rows_with_omissions`, uses companion order,
excluding only the actual source-bound generated primary. Its
`IncrementingAutoNumber` policy remains explicit and retains the exact original
LAST precondition. Omission never selects a generation mode or identifies a
generated column. NewValues remains unqualified.

Existing complete `RowInsert`, `AutoNumberRowInsert` and preview methods retain
their complete-value contracts. Source qualification runs before omission
resolution. Every original table/index/relationship, carrier, validation,
active-content and allocation gate remains applicable; no normal ACE save or
journal capability is introduced.

The initial needed-default profile accepts literal NULL, Boolean, quoted text,
signed integers and plain decimal literals, including parentheses and unary
numeric signs. Text uses the target's exact text carrier. Integers use checked
native widths; Currency uses its exact scale; Decimal uses exact coefficient
rescaling and the field's declared precision/scale. Decimal values never pass
through binary floating point, and nonexact rescaling or overflow refuses.
Display DecimalPlaces does not determine storage scale.

This profile does not infer general Variant coercion. Numeric-to-Boolean,
text-to-number, floating/date literal defaults, arithmetic expressions, numeric
suffixes/exponents, calls and references remain outside the initial needed
profile. Unsupported expression contents remain inert when their field is
supplied, even when another field in that row needs a supported default. Invalid
property scope/carrier/flags retain their existing admission refusals.

A present empty DefaultValue expression is not treated as an absent declaration.
Needed defaults must fit the bounded literal parser; no SQL statement is built
by interpolating property text. No field reference, parameter, domain, external
source, UDF, Eval, clock or VBA callback is invoked. Defaults on generated or OLE
fields remain outside the qualified declaration profile.
[DefaultValue property](https://support.microsoft.com/en-us/access/defaultvalue-property)

After resolution, the actual record, Memo/OLE, DATA and physical-index builders
consume complete typed values once. Required, AllowZeroLength, validation,
precision/scale, primary/unique and foreign-key checks still decide whether that
complete row is admissible. A missing Boolean without a default is not silently
False; NULL retains its actual carrier refusal. A conflicting default key is not
repaired, skipped or replaced with a live maximum.

## Original intent and independent proof

The coordinator retains original presence plus the explicit generation policy.
Producer-expanded values and a cached excluded-column position are not audit
authority. The audit separately reads source property scopes, associates each
needed default with the actual field, derives expected values and reconstructs
the complete record/key/allocation image. Its numeric projection is independent
of the producer's assignment/rescale implementation. Only the existing bounded
parser and paid carrier-copy primitives are shared.

Generated inputs additionally use the existing independent counter audit. It
rederives actual primary selection, signed TD20, checked +1 keys and the final
counter from source and the original policy. Complete before/after images and
returned keys must agree before a candidate is returned. No source is changed.

## Literal coverage and current verification

The [ordinary source owner](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/omitted_defaults.rs)
and [Decimal source owner](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/decimal_index_inserts/omitted_defaults.rs)
reuse existing independent MR2/record/index/image builders. They construct 30
complete expected images before new APIs: 24 ordinary, two incrementing and four
Decimal cases across both ACE 16 layouts. They cover omitted constants, nullable
fallback, explicit NULL/value/empty, an unneeded unknown default beside a needed
constant, a non-first generated primary, exact Decimal scale 4 and 28, and
readable Required/unique/validation/domain refusal sources.

The first source run passed incrementing and Decimal controls but rejected a new
text-comparison validation declaration as unqualified. Before any resolver/API
ran, that refusal source was replaced with a supported numeric ID rule `>0` and
default `-1`; existing source IDs 1/2/3 remain valid. The corrected gate explicitly
requires checked validation coverage. All three source functions then pass.
The original source and failed log remain retained; positive expected images
were unchanged.

The first actual API run passes all ten focused functions: the three source
functions, five ordinary/Decimal API functions and two causal audit controls.
All 30 complete outputs match. Typed refusals, stale/exhausted generation policy,
empty operations, all budget pools and measured terminal cancellation pass.
The causal controls first match independently built healthy and alternative
images, then reject a poisoned expanded cache/plan/candidate and a correlated
replacement of supplied NULL with a default.

Named schema/Core precision, scale and Currency constants replaced identical
numeric literals after that first pass. SQL selection and CLI publication use
separate adapter contracts and qualification.

The combined regression round passes 498 native mutation functions and 72 tests
across ten shared native integration targets. Access and query library rounds
pass 51 and 84 functions; the affected CLI round passes 31 unit and 33 integration
functions. Strict Clippy for five packages and minimal ACE/Access builds pass.
Scoped formatting and documentation-link checks pass. The first formatting
check required sorting two test module declarations; only their order and a
production module comment changed afterward. No native Access open/save or
original-file default mutation qualification is claimed.

## Intended extensions

The additive [captured-clock previews](ACE16_CLOCK_INSERT_DEFAULTS.md) now resolve
needed Date()/Now() through an explicit operation input. The literal-only methods
above retain their no-clock behavior and refusal. Opening or inspecting metadata
never reads ambient time or executes defaults. Other constant forms/coercions,
deterministic defaults, DEFAULT VALUES and zero-field SQL forms need their own
qualification; this profile is an initial implementation, not a permanent
compatibility boundary.
VBA/runtime execution and normal-save/data-macro qualification remain separate.
