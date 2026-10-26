# Integer logical operators and numeric predicates

The expression library now evaluates exact integer `Not`, `And`, `Or`, `Xor`,
`Eqv` and `Imp` without converting operands to Boolean first. Thus `1 And 2`
produces integer zero; `Not 1` produces -2. Both written operands remain eager.
The non-Text kernels allocate no payload buffers and introduce no dependencies.
Contextual Text conversion uses the existing metered normalization buffer.

`logic::Logical` and `logic::complement` are parser-independent kernels. The SQL
operator evaluator uses them in source-free, ordinary, grouped, derived, domain,
correlated and collection queries. Existing Boolean/NULL truth tables remain
unchanged. Stored `Value` gains no language-only states.

## Carriers and NULL

Byte/Byte binary operands produce Byte. Otherwise Int64 dominates Int32, Int32
dominates Int16, and Byte mixed with Int16 or Boolean produces Int16. Boolean
true contributes signed -1, false zero; two Boolean operands return Boolean.
Unary complement retains its Boolean or integer carrier. Byte complements wrap
within eight bits; narrow signed results retain their own width and mixed signed
operands extend their sign before combining bits.

Mixed integer/NULL binary operands use the integral tables in Microsoft's
[And](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d53142ee-9ac2-4ecf-bc24-d7167b806eb3),
[Or](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8067b684-0a62-4860-840e-4a20abc10c02)
and [Imp](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9d1d4bb9-7e95-4d53-8f3c-b696e4924b56)
sections: AND zero yields typed zero, OR retains the known integer, and IMP
retains the documented known complement/right operand where applicable. XOR/EQV
with NULL yield NULL. Results use the known operand's integer carrier. In
particular, integer `0 Or NULL` retains integer zero, while Boolean
`False Or NULL` retains the existing unknown result. These distinct provisional
typed policies require native Access comparison; do not silently equate their
subtypes or describe specification-derived controls as Access evidence.

The [logical operator type tables](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/2d70780d-6e99-47d6-9759-2b3a46b8e862)
and [Not definition](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/16b00902-24c6-4b52-937e-61fae205557f)
are primary references. This is an explicit materialized carrier profile, not
complete Variant subtype inference. Rounded numeric and contextual Text coercion
now use the profile below. Error operands now fail TypeMismatch after eager
binary evaluation; see [Error predicates and logical rules](../VARIANT_ERRORS.md#error-predicates-and-logical-operators).
Host Empty now follows the explicit admission rules below. Missing, Extended
dates and native subtype inference remain unsupported.

## Expression data and eager String conversion

The logical operator boundary now admits host-supplied Empty as Integer zero.
`Not Empty` therefore returns Int16 -1. Empty mixed with Byte or Boolean uses
the Integer carrier, rather than manufacturing Byte zero or Boolean False;
Long/LongLong and rounded numeric/String inputs retain their existing wider
selection. Empty mixed with NULL follows the existing Integer/NULL tables.
This implements the Empty rows in Microsoft's linked logical effective-type
tables without introducing stored Empty, an SQL Empty constructor, a VBA host,
or complete native Variant inference.

Both evaluators now retain raw String operands until both written logical
expressions have evaluated. For example, `ChrW(55296) And (1/0)` raises the
right expression's DivisionByZero before attempting to publish the left
unpaired UTF-16 unit. Raw Strings in derived, grouped and correlated cells use
the same boundary. This fixes eager operand evaluation; it does not short-circuit
logical conversion. `ChrW(55296) And NULL` and `0 And ChrW(55296)` still fail
strict Unicode publication because logical operands must be converted even
when known bits could determine a result.

After eager evaluation and expression-state admission, numeric conversion runs
in written order. A left overflow or missing number policy remains authoritative
before a malformed right String. Valid raw numeric Strings use the existing
checked Text parser, selected from original operands: an Int64 participant keeps
exact LongLong conversion, including integers above Double's exact range.
Error remains TypeMismatch and Missing remains NonScalarExpression; neither is
coerced to zero. Unary and binary results retain their original Scalar/Variant
provenance in query composition. Text/byte/work allowances and cancellation are
cumulative, including Empty-only direct calls. No system locale, callback,
dependency or native Access qualification is added.

Focused tests cover all five binary operators and Not, both Empty operand
orders, typed NULL rules, exact large numeric Strings, eager malformed-String
errors across query scopes, conversion precedence, declaration-sensitive later
arithmetic, resource exhaustion and every observed cancellation checkpoint.
These are specification-derived engineering controls; Windows Access execution
remains unrun.

The original [retained String replay](../../testing/compare_retained_logical.py) passes
2,056 CLI invocations: 3,448 successful value/subtype controls and 320 buffered
refusals. Decimal nearest-even rounding and independent per-bit signed 32/64-bit
truth tables supply the expected answers for both CLI evaluators. Coverage includes
LeftB/RightB/MidB, repaired nested slices with an odd intermediate, all six
logical operators, en-US/de-DE Text, exact signed LongLong boundaries, typed NULL
tables, eager query scopes, conversion-order diagnostics and declaration-sensitive
widening. The [recorded report](../../testing/results/retained-logical-2026-10-24.json)
retains the counts and CLI SHA-256, unchanged before/after the run:
`5d2c57f043b15264c0d59395e6f0940d6b73b344cbff4884df90e08c2650f9b6`.
No database fixtures, third-party resources or native Access execution are used.

## Rounded numeric and Text operands

Single, Double, Currency, Decimal and legacy DATE operands now reuse the existing
checked half-even integer conversion kernels before applying their bits. Unary
Not converts these inputs to Long (Int32). Binary logical operators convert both
non-NULL inputs to Long, or LongLong if an original input was Int64. Boolean and
Byte values mixed with these inputs enter that same signed target width. Two
ordinary integer/Boolean inputs retain the exact-width policy above.

For example, `CCur(2.5) And CByte(3)` produces Int32 2 and `Not 2.5` produces
Int32 -3. A raw DATE serial of -1.25 rounds to -1 before complementing, independently
of its civil calendar interpretation. Decimal/Currency rounding retains the exact
coefficient; no new decimal arithmetic is introduced. Int64 promotion likewise
preserves values beyond Double's exact integer range. Finite/range checks happen
before bitwise combination, so a zero/NULL input never suppresses overflow or
non-finite conversion errors in the other input.

SQL evaluators additionally convert numeric Text under the explicitly selected
NumberFormat. They choose the target width from the original operand carriers,
then normalize/round Text directly through the existing checked integer parser.
No intermediate Double loses large Text integer precision. Missing policy fails
with NumberFormatRequired; malformed and out-of-range inputs fail explicitly.
Pure parser-independent kernels still reject Text rather than invent a policy.
Numeric Text is required for operator conversion: Boolean words accepted by
CBool predicate conversion do not automatically become integer operator inputs.

Text scans, normalization allocation and cancellation use the execution's shared
budgets. Both written binary expressions evaluate before operator conversion;
conversion consumes operands left to right and stops immediately on operational
failure. The conversion buffers share cumulative limits; AND zero does not bypass
the second conversion. No clock, system locale or payload tracing is added.

The current parser's integer literal carrier remains Int64. Consequently a
literal integer can select LongLong promotion where explicit CInt/CLng or native
Byte/Int32 fields would select Long. This existing literal/subtype inference gap
remains native qualification work. Microsoft's static logical target tables guide
this explicit materialized profile; they do not establish the complete runtime
Variant effective-type tables or native Access SQL behavior. Wine's
[OleAut32 variant implementation](https://github.com/wine-mirror/wine/blob/master/dlls/oleaut32/variant.c)
was also consulted as a secondary coercion reference; no code, tests or fixtures
were copied and no Wine/Windows integration is introduced.

## Final predicate boundary

`Truth::from_value` remains strict Boolean/NULL conversion. The separate
`Truth::from_predicate` accepts numeric and legacy DATE zero/nonzero using the
existing checked Boolean conversion. Exact integers, Currency and Decimal retain
precision; finite fractional values are tested before integer rounding. NULL
remains unknown and does not select a row. The evaluator's `predicate_truth`
additionally applies execution-local CBool Text conversion, with the explicit
number policy and cumulative budgets. For instance, Text `0,25` under de-de
selects a row even though integer operator conversion would round it to zero.
Boolean words use that same CBool policy. Pure `Truth::from_predicate` still
rejects Text. GUID, Binary, Extended dates and expression-only states have no
implicit predicate contract.

Read-only WHERE/HAVING/ON, including crosstab filters, collection members and
joined collection scans, apply that final predicate conversion. They preserve
integer results until the complete scalar expression has evaluated. UPDATE's
separate final predicate check remains Boolean/NULL; shared scalar expressions
can now compute the documented exact integer operators inside its existing
qualified edit profile. No new native field write type or writable format is
introduced.

## Collection criteria

Parent criteria retain scalar intermediate values and call the same operator
kernels as ordinary retrieval. Binding splits independent Boolean memberships
only when their item results are provably Boolean; scalar-only operands may
retain integer carriers. Numeric member operations execute before their
membership fold. For example, `Tags.Value And 1` tests each item's low bit: an
item value of 2 alone does not match. `Not Tags.Value` likewise complements each
item before testing its result.

`Tags.Value=7 And Tags.Value=8` still permits independent matching children.
Nested scalar bit masks combined with those Boolean memberships retain that
behavior. Unproven member result types stay in a complete per-item atom rather
than being silently reduced to Boolean inputs. Classification is syntax-only,
metered and depth-limited; it does not scan source rows. Existing product limits,
NULL/empty/outer-absence policies, eager failure and buffered publication remain.
Broader native collection expression semantics remain unqualified.

## Verification

The updated expression/query batch passes 796 tests including doctests, with 302
separately run parser-free expression tests and five selected CLI tests. All-feature and minimal Clippy,
warnings-denied rustdoc and formatting checks pass.

Focused tests cover widths, sign extension, typed NULL rules, unchanged Boolean
truth, strict versus final predicate conversion, joins, groups, crosstabs,
correlations, eager errors and per-item versus parent membership. Proptest
compares all five Int64 binary operators against independent per-bit truth tables;
generated collection controls use parity and independent membership models.

The original [independent runner](../../testing/compare_integer_logic.py) passes 1,394
integer/predicate/eager/native-field controls. Its per-bit model covers all four
integer widths through both CLI commands. Read-only Jackcess getters supply ID
references for unchanged Jet 4, ACE 2007 and ACE 16 2016/2019-family fixtures;
TableCraft applies the actual query operators. Source hashes are checked before
and after. These are independent storage/integration controls, not native Access
SQL execution. No third-party fixture or code is added by this change.

The original [rounding comparison runner](../../testing/compare_logical_coercion.py)
passes 2,658 independent controls using Decimal nearest-even expectations,
per-bit tables and exact binary quarter values. It checks both CLI commands,
explicit en-us/de-de policies, exact large-Text promotion, eager failures and
read-only native ID/DATE getters on the same four unchanged source families.
These controls do not execute or qualify Microsoft Access SQL. The earlier
1,394-control integer runner also passes with its former unsupported fractional
Not case replaced by a genuine overflow refusal.

## Declared arithmetic after logical results

Bound queries now retain declaration categories through all six logical
operators under the explicit `--declared-arithmetic` profile. Scalar operands
retain Scalar; unary Variant retains Variant, and either binary Variant operand
establishes a Variant result. This affects subsequent overflow/widening decisions,
not the logical kernels or default results. Unknown aggregate declarations stay
unknown. See [the reviewed static tables and scope](DECLARED_ARITHMETIC.md#logical-operator-declaration-propagation-23-october-2026).


## Query output metadata

The query binder now publishes [stable logical result descriptors](../../query/QUERY_RESULT_TYPES.md#logical-result-carriers-and-nullable-alternatives)
without executing operators or reading sources. NOT preserves successful carrier
and nullability under this profile; binary operators consider possible NULL
substitutions before claiming a Boolean/integer carrier. Nullable inputs that can
change the successful carrier remain Dynamic. XOR/EQV NULL-only results describe
successful values only and do not skip operand conversion or eager failures.
Known descriptors now feed existing UNION widening, while native Scalar/Variant
provenance remains independent. Operator values, rounding and native qualification
limits are unchanged.
