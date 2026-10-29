# Reviewed read/query P0 cases

This inventory separates remaining implementation scope from deliberately refused
cases and missing native evidence. It refines the two P0 rows in
[read/query acceptance](READ_QUERY_ACCEPTANCE.md); it does not replace the ACE16
compatibility target or declare the rest of read/query complete.
This is a bounded inventory of reviewed cases, not an exhaustive coercion or
locale matrix. Function-specific regional and conversion gaps remain in their
owning contracts and the built-in tracker.

Reviewed 26 October 2026 against the current owning contracts and code. The
initial inventory added no Cargo or Windows Access execution. The subsequent
French legacy stored-index oracle is recorded separately below; it is independent
reference evidence, not implemented support or native Access qualification.
Other examples remain contract discriminators rather than a newly executed
matrix. Existing records retain their original scope and hashes.

## Classification and decision rule

- **Demonstrated implementation gap:** an admitted operation disagrees with an
  established rule, or a required capability is explicitly absent in code. A
  missing capability is not automatically a demonstrated wrong answer.
- **Conservative refusal:** the engine explicitly declines a case whose category,
  provider or storage semantics have not been established. Removing the guard
  requires evidence; a plausible result is insufficient.
- **Native qualification pending:** a working engineering rule needs actual ACE16
  execution observations before it can be called native-compatible.

The finite scalar and text reviews found no additional demonstrated wrong-answer
case requiring a semantic patch. Existing aggregate/UNION category ambiguity
and native text uncertainty must not be relabeled as bugs to keep a queue full.
Additional stored sort orders are a concrete missing implementation family.
The exact French legacy descriptor now has an implemented closed physical-read
profile and a stored-key/reference oracle. A separate provisional full-string SQL
comparison provider and bounded Database-mode substring search are implemented.
LIKE, Text/LCID modes, other sort families and native qualification retain
separate gates. The substring implementation was added on 28 October 2026;
the initial review and its evidence classifications remain unchanged.

## Value categories and declared arithmetic

| ID | Concrete discriminator | Working behavior and classification | Next acceptance gate |
| --- | --- | --- | --- |
| V1 | `SELECT First(CInt(2))+CInt(1)` versus `SELECT First(CInt(32767))+CInt(1)` under declared arithmetic | The first returns Integer 3 through Scalar/Variant agreement. The widening case refuses `ArithmeticDeclaration`; the aggregate category is unknown. **Conservative refusal**, not missing ordinary addition. | Capture actual ACE16 result carriers and downstream category-sensitive outcomes for First, Last, Count, Sum, Avg, Min and Max separately. Do not infer a category from one nonempty result. |
| V2 | `SELECT '02'=First(CLng(2))` under declared arithmetic | Refuses when unknown aggregate provenance changes String-versus-number comparison dispatch. Explicit CVar or a typed cast supplies a category. **Conservative refusal.** | Distinguish native String/numeric coercion with leading zeros, both operand orders, NULL/empty groups and explicit-cast controls. Record build and number format. |
| V3 | A UNION result used in a later widening operation, including an empty arm | UNION conversion and paired result descriptors work; they do not establish the resulting Scalar/Variant declaration. **Conservative refusal** where branches differ; **native qualification pending** for category assignment. | Capture each written arm order and prefix stage, including an empty leading/trailing arm, then observe downstream overflow and String comparison. Never choose a category from the first row. |
| V4 | `SELECT 32767+1`, `CVar(32767)+1`, and `(NULL Or CInt(32767))+CInt(1)` under declared arithmetic | Literal widths, known Variant widening and literal-NULL Variant provenance are implemented. These are regression controls, **not residual implementation gaps**. Native ACE declaration equivalence remains separate. | Preserve the existing implementation records; add native observations only when available. An operational error common to both declaration branches must not become category ambiguity. |

The [declared arithmetic contract](../expressions/numeric/DECLARED_ARITHMETIC.md#binary-arithmetic-with-unknown-provenance)
and [successful-result metadata](../query/QUERY_RESULT_TYPES.md#successful-arithmetic-with-unknown-declaration-provenance)
define V1–V4. The implementation is in `tablecraft-expressions/src/numeric` and
`tablecraft-query/src/{binding/result_types,plan/declarations,execution/declarations}`.
Existing `declared_integral`, `declared_result_widths`, `relational_declarations`,
`declared_string_comparisons` and `null_literal_declarations` targets cover these
boundaries. The [agreement result record](results/agreement-result-types-2026-10-25.json)
and [Scalar String/Variant evidence](SCALAR_STRING_VARIANT_COMPARISONS.md) are
engineering evidence, not ACE result-category observations.

Microsoft's [relational effective-type table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
distinguishes declared types from runtime value types. The
[Access Count reference](https://support.microsoft.com/en-us/access/functions/count-function)
describes counting semantics; it does not supply the missing downstream
Scalar/Variant provenance for every SQL aggregate. Language-level rules do not
by themselves establish ACE's category assignment to a SQL aggregate or UNION.

## Error data and aggregates

| ID | Concrete discriminator | Working behavior and classification | Next acceptance gate |
| --- | --- | --- | --- |
| E1 | Count/First/Last over `CVErr(1)` and NULL | Count counts non-NULL Error data; encounter aggregates retain the selected cell. Scalar-only publication refuses retained Error; expression publication supports it. **Working**, with native Error aggregate behavior **qualification pending**. | Observe native result, IsError/VarType and NULL behavior independently for each consumer; distinguish operational failures from stored expression Error data. |
| E2 | Min/Max over standard Error codes 0, 1, 65535, and a mixed Error/non-NULL scalar population | Standard Error-only extrema order by code; mixed populations refuse TypeMismatch. This is an explicitly provisional engineering profile. **Native qualification pending**, not an agreed missing numeric-code coercion. | Capture both encounter orders, NULL-only/empty groups, duplicate codes and a later operational failure. A native disagreement becomes a narrowly scoped implementation case. |
| E3 | `SELECT Sum(CVErr(1))` and the explicit-code control `SELECT Sum(CLng(CVErr(1)))` | Implicit Error admission refuses TypeMismatch; explicit supported code conversion permits numeric aggregation. **Conservative refusal/native qualification pending** for broader Error aggregate semantics. | Compare Sum/Avg and statistical/domain consumers separately. Do not silently unwrap Error codes or generalize Min/Max's provisional rule. |
| E4 | Error group/set/sort keys versus an opaque implementation-defined Error code | Standard-code relational keys retain Error identity; opaque codes lack a supplied ordering. **Working standard-code profile/conservative opaque-code refusal.** | Native group/set/order captures must establish whether the same profile applies; numeric equality or the stored core Value representation cannot serve as the oracle. |

See [expression aggregates](../query/aggregates/EXPRESSION_AGGREGATES.md),
[Variant Error data](../expressions/VARIANT_ERRORS.md#standard-error-relational-keys)
and [strict keys](../query/keys/NUMERIC_KEYS.md). Owning code includes
`execution/aggregate`, `execution/keys.rs` and the expression comparison/conversion
services. Existing `error_predicates`, aggregate/domain and key tests are the
regression surface. Native Access Error-cell aggregate ordering is explicitly
unqualified; the finite audit did not establish a new Error aggregate bug.

## Text-policy, stored-order and function-provider cases

| ID | Concrete discriminator and required context | Working behavior and classification | Next acceptance gate |
| --- | --- | --- | --- |
| T1 | `SELECT LegacyText=ModernText FROM PolicyProbe` with the required field resolver and different retained identities | Conflict is explicit; an explicitly selected global override has its separate supported behavior. Runtime domain/aggregate/constructed String provenance is retained. **Working host policy; native mixed-policy precedence pending.** | Execute the same per-field originals in ACE16 for direct/reversed comparisons, derived aliases, NULLs, aggregates and explicit overrides. Record database and field descriptors separately. |
| T2 | `LegacyText & IIf(True,2,ModernText)` subsequently compared or grouped | Converted numeric data does not contribute the unselected Text field's policy; actual String and conservatively retained NULL policies do. Completed construction is not overwritten by static fallback. **Working**, not a generic lost-policy TODO. | Keep the existing late-contributor/cohort and provider-activation controls. Add a bug only with a discriminator that violates this stated host contract. |
| T3 | A stored Text field with an exact non-General descriptor, or a nonzero General variant/unknown table version, followed by comparison or selected text-index traversal | NativeSortOrder preserves the descriptor. General services and exact ordinary French legacy physical reads implement closed profiles; separate French full-weight comparison and bounded Database-mode substring search are implemented. French LIKE, other orders and native qualification remain unfinished. **Concrete missing provider/key families with conservative refusals**, not permission to substitute ICU/ordinal or ignore descriptor bytes. | French legacy `1036/0/0` has unchanged stored-key and independent getter/order evidence; its closed ordinary single-field physical encoder/read profile is implemented. Full-string comparison has separate short-key/long-Latin evidence; substring search has independent window-oracle and bounded-work engineering checks. Native scalar comparison/matching and French LIKE remain separate gates. Other orders still need their exact descriptor/oracle gate. |
| T4 | `StrComp('A','a',1)` or `InStr(1,'xI','ı',1055)` with explicit ICU locale/LCID registration | Nineteen explicit ICU host profiles and one exactly registered LCID per execution already work. **Working host implementation; NLS/ACE equivalence pending.** | Capture function-specific ACE16/NLS results with build, locale, flags, Option Compare and full LCID identity. Do not turn an LCID's structural validity into automatic provider support. |
| T5 | Binary StrComp, General database comparison, LIKE and substring matching on trailing spaces/raw UTF-16 | Distinct current contracts already exist: binary units, General SQL padding, pattern spans and bounded search callbacks. **Working engineering profiles; native function-specific padding/matching pending.** | Compare each consumer independently using empty/NUL/trailing-space inputs, expansions, surrogate boundaries and consumed spans. Equality agreement alone does not qualify LIKE or search. |

The [runtime policy contract](../query/RUNTIME_TEXT_POLICIES.md) specifies T1/T2;
`execution/collation{.rs,/provenance.rs,/declared_strings.rs}` and
`execution/declarations/predicates/{like,range,membership,subquery}.rs` route these
consumers. Existing `collation_routing` and retained-list policy tests cover
construction, dynamic cohorts, callbacks and named/derived propagation. The
[original per-field fixture ledger](FIXTURES.md#native-per-field-general-policy-inputs)
records 172 independent model/source controls, not native mixed-order SQL results.

For T3, inspect `tablecraft-ace/src/collation/sort_order.rs`,
`table/indexes/text.rs` and
`tablecraft-access/src/collation/resolver.rs`. Exact retained descriptors and
explicit refusals already have tests. No new native-sort implementation was
approved by this inventory. In particular, read support or ICU language names
cannot establish a compatible persisted key representation.

The [ICU profile](../expressions/text/ICU_TEXT_PROFILE.md) records 14,498 controls
across nineteen profiles against its stated independent ICU4C model. It explicitly
separates stored database/LIKE routing and NLS equivalence. Microsoft's
[sorting guidance](https://learn.microsoft.com/en-us/windows/win32/intl/handling-sorting-in-your-applications)
also distinguishes linguistic/ordinal sorting, locale selection and sort options;
it does not establish interchangeability with ACE's persisted key tables.
[StrComp](../expressions/text/STRING_COMPARISON.md),
[search](../expressions/text/STRING_SEARCH.md) and
[LIKE](../expressions/text/PATTERN_MATCHING.md) retain their owning boundaries.

## What can advance next

T3 is a real missing provider/physical-key family, rather than an unimplemented
ICU callback. The [French legacy oracle](FRENCH_LEGACY_INDEX_OBSERVATIONS.md)
now establishes exact descriptor `1036/0/0`, independent stored keys and distinct
accent ordering on unchanged ACE16 V2016/V2019 originals. Both profiles pass
456 logical index memberships, or 304 physical entries after alias deduplication.
This satisfies the descriptor/oracle entry gate for that closed family; physical
encoder/read support is now implemented under its [closed contract](../access/indexes/NATIVE_TEXT.md). A [separate provisional full-string query provider](FRENCH_COMPARISON_OBSERVATIONS.md)
now supplies SQL/default/Database ordering, including ordinary Memo values. This
does not qualify native Access comparisons, equality-prefix hints, LIKE,
Memo physical indexes or mutations. The subsequent
[French Database substring service](../access/indexes/FRENCH_SEARCH.md) implements
complete-weight consumed spans with primary-only pruning. Its first seven
kernel, one invariant and thirteen Access functions pass, along with thirty
General regressions. Native substring matching remains unqualified; this does
not close T3 or P0.
Other orders still need their own admissible oracle. Do not derive a stored order
from an arbitrary locale library or relabel a host profile as native compatibility.

V1–V3 and E1–E4 have finite capture gates. Until category or aggregate evidence
resolves them, conservative refusals remain necessary. The
reviewed Val Single-rounding observation likewise does not establish a defect:
the current [Val contract](../expressions/numeric/VAL.md) specifies its narrowing
path, and a different direct lexical rounding route needs additional evidence.

No independently proven wrong-answer patch emerged from the finite scalar/text
audit. Backend implementation can continue against its documented contracts
while these evidence gates remain explicit. P0 stays open. Completing an
additional streaming or collection shape does not close these semantic cases.
