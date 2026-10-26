# Eager conditional expressions

## Lossless IIf branches

IIf now retains raw String branches until an owning consumer requests publication.
Both branches still evaluate once in written order before the condition is
converted and selected. An unselected raw surrogate or odd byte range is valid
String data; it is not itself an operational failure. Thus
`IIf(False,ChrW(55296),'ok')` returns ordinary Text, while
`LenB(IIf(True,ChrW(55296),ChrB(65)))` returns 2 without decoding the selected unit.
An invalid constructor, division failure, cancellation or resource failure in
either branch still aborts evaluation. A selected unrepresentable String still
fails when published directly as a scalar cell.

The existing allocation-free IIf selector now moves a generic already evaluated
payload. Scalar callers retain ExpressionValue behavior; lossless callers retain
the raw buffer without cloning or coercion. Condition conversion and Empty/Error/
Missing rules are unchanged. Bound execution merges both branch text policies and
retains IIf's Variant declaration, including grouped runtime provenance. Type
inspection, byte/unit observations, NULL tests, concatenation and Count can consume
the selected raw String through their existing services.

This closes the IIf branch-publication gap, not raw relational storage, arbitrary
conditional functions, result promotion or native Access qualification. Nz,
Switch and Choose now also retain lossless choices as described below. The original
[IIf runner](../testing/compare_raw_string_iif.py) passes 1,392 independent source-free
selection/refusal controls in both CLI modes, using Python byte selection rather
than TableCraft's own evaluator as the oracle.

## Lossless Nz, Switch and Choose choices

Nz now tests NULL identity over lossless operands and moves the input or eagerly
evaluated replacement without decoding either String. An omitted query replacement
still yields empty Text, not host Empty. Thus `LenB(Nz(NULL,ChrB(65)))` returns 1,
and `Nz('ok',ChrW(55296))` returns valid Text. Invalid constructors and operational
failures in the unused replacement remain visible.

Switch/Choose now separate control expressions from owned choice payloads through
the same incremental selector. Control coercion, deferred semantic errors, immediate
operational failures and eager written-order evaluation retain their existing
contracts. Only one selected payload is retained; other evaluated choices are
dropped without a payload-vector allocation. A selected NULL remains distinct from
no match, so later Switch pairs cannot replace it. Choice payloads may contain raw
units, odd bytes, ordinary values or expression Error data without implicit conversion.

The default Selection API retains ordinary ExpressionValue behavior. Its generic
payload API validates control/choice slot order, latches ignored structural errors
and returns Option for no match; callers supply their own NULL representation.
Evaluator entry points retain shared work/cancellation and existing Text control
policies. Bound calls merge choice policies, preserve Variant declaration and
enrich grouped runtime provenance. Direct final publication remains strict; raw
control expressions now share the retained-control admission below; relational
cells retain their separate storage limits.

The original [selection runner](../testing/compare_raw_string_selection.py) passes
1,806 independent source-free controls in both CLI modes. Existing IIf, raw NULL
and Count recipes also pass. Native Access execution/result promotion remains
unqualified; this supplies no VBA runtime or stored raw-string carrier.

## Retained String controls

Evaluator-backed `IIf`, `Switch` and `Choose` now accept valid active String ranges
through the same strict, metered Unicode publication and explicit Boolean/Single
conversion as materialized Text. Both scalar SQL evaluation and bound query
execution retain raw controls until their owning conversion. This does not admit
malformed Unicode as numeric or Boolean data.

`IIf` evaluates both branches before converting its condition. Switch/Choose latch
ordinary control-conversion failures, including unrepresentable retained Text,
until the remaining expressions have evaluated; a later division error therefore
remains visible. Resource, allocation, Text limits and cancellation remain immediate
operational failures. Invalid String constructors fail during expression evaluation.
Public evaluator selector APIs use the same conversion service and preserve ignored
operational-error latching. Choice carriers, declarations and text-policy provenance
are unchanged. Native Access error precedence remains unqualified.

## Switch and Choose control rules

`tablecraft-expressions::functions::conditional` owns closed Switch/Choose
identities and an incremental owned-payload selector. Scalar evaluation and retrieval use
the same selector. No VBA/runtime/provider dependency is introduced.

Switch accepts condition/value pairs and returns the first true condition's
value. A selected NULL stays NULL even if a later pair is true. When no condition
is true the result is NULL. Conditions share IIf's explicit numeric/Boolean policy:
zero and NULL are false, nonzero exact/finite numeric values and legacy DATE
serials are true. Evaluator-backed calls also accept Text under the execution's
explicit NumberFormat, using the same Boolean conversion as CBool. Binary, GUID,
Extended dates and Error/Missing conditions remain unsupported.

Choose accepts an index and one or more choices. Indices are one-based and use
checked Single conversion followed by half-even integral rounding, including
legacy DATE serials and explicitly formatted numeric Text. A NULL or representable
out-of-list index returns NULL. Finite Single magnitudes beyond Int64 still return
NULL because they cannot address a listed choice. Inputs outside Single's finite
range report Overflow; invalid types/non-finite inputs retain typed failures. Returned choices retain their exact carrier and payload; no common
result type or implicit conversion is inferred.

## Evaluation and limits

Both functions evaluate **every supplied expression in written order**, including
unselected choices and later Switch conditions. Selection retains only one
chosen value and drops other evaluated payloads, without allocating or cloning an
argument-value buffer. The selector's `push` consumes values; `finish` requires
the declared argument count. Extra-argument errors are latched even if a caller
ignores the returned error. Selector state deliberately omits Debug/Serde.

Ordinary semantic truth/index conversion errors are deferred until all arguments
have been evaluated. Resource, cancellation, allocation and other operational
failures abort immediately and remain latched; they never permit later argument
callbacks after failure. A later expression evaluation error therefore remains visible even
when the selector already has a conversion error or chosen value. Invalid later
Switch conditions are not hidden by an earlier match. This is the implemented
eager-error contract; native Access error precedence still needs comparisons.

Switch requires at least one complete pair; Choose requires an index and at least
one choice. Missing argument slots, unsupported qualifiers and invalid arities
fail before expression evaluation. Query binding resolves *all* operands even
when their branch would be unselected, without row scans. Grouping/aggregate
validation, parameter rebinding, correlated-slot rewriting, projection pruning,
DISTINCTROW source dependencies and crosstab expressions traverse every operand.

The pure selector does constant work per argument and owns no I/O. Contextual Text
conversion scans/normalizes through the existing metered numeric converter.
Callers supply their work limits; the scalar/query evaluators retain normal node/depth,
allocation, text and cancellation budgets. Unselected text allocations still
consume budget. Results publish only after successful complete evaluation.

The public evaluator `apply_iif` and `push_selection_argument` entry points also
check cancellation/work for non-Text conditions and choice payloads. A last,
unselected binding callback that requests cancellation cannot return a successful
chosen result. Pure `Selection` still requires caller-owned metering. Evaluator
operational failures are latched even if the caller ignores the returned error.

## Evidence and remaining qualification

Microsoft's [Switch reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/switch-function)
and [Choose reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/choose-function)
define pair selection, rounded one-based indexing, NULL outcomes and eager
expression evaluation. Their rules guide implementation; they do not qualify
native return-type promotion or error ordering.

Four authored/generated value tests cover first-match/NULL behavior, mixed
carriers, exact/half-even/out-of-range indices, invalid state transitions and an
independent vector-selection model. Scalar dispatch/order and unselected-text
budget tests cover eager host calls and late failures. Five retrieval tests cover
binding, pruning, mixed carriers, grouping, aggregates, parameters, correlations,
crosstabs, errors and cancellation. The [CLI recipe](../testing/compare_conditional_functions.py)
compares independent selection models and registered sequential/indexed native
rows while checking unchanged source hashes. It handles existing quoted Text
presentation through the TSV display adapter; it does not change CLI formatting.

Extended-date and complete Variant/locale coercion, native SQL return-carrier
promotion, NULL index
and error-precedence qualification, and actual Windows Access comparisons remain
open. No native Access SQL execution evidence is claimed.

## Expression Error data

[CVErr Error data](VARIANT_ERRORS.md) now survives eager IIf/Nz/Switch/Choose
selection for explicit consumers. An unselected Error data value can be dropped;
an unselected failing expression still fails. IIf condition conversion waits
until its evaluated branches are available, and Switch/Choose retain their
deferred semantic condition/index error policy. Selected Error data uses the
[expression-valued result/derived-row profile](VARIANT_ERRORS.md); checked scalar
publication and storage boundaries still reject it. Error codes never become
implicit condition/index numbers. Expression arrays remain outside this profile.

## Date, Text and explicit expression states

Legacy DATE conditions test the stored serial against zero without integer
rounding or calendar conversion. Positive/negative fractional serials are true;
either signed zero is false. Choose first uses the shared checked Single
conversion, then rounded indexing, retaining one-based/out-of-list rules. Native DATE schemas, stored
values and selected branch carriers remain unchanged. The
[DATE coercion specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8a4a9201-4e7f-4856-b9fc-5927d2879723)
provides the numeric/Boolean representation basis.

Evaluator::apply_iif and Evaluator::push_selection_argument provide execution-local
Text conversion for scalar, bound, nested, domain and closed Eval paths. The CLI's
existing `--number-format invariant|en-us|de-de` supplies that policy; omission
returns NumberFormatRequired for evaluated Text condition/index slots. Pure
materialized helpers still require already converted scalar inputs for Text.
Selected Text payloads are never parsed or reformatted. No default locale,
collation or native text policy is inferred from a condition.

The converter normalizes under the shared byte/work/Text limits before fallible
allocation. Missing-policy and malformed/unsupported conversion failures retain
the selector's eager semantic error handling. Text Choose indexes share the same Single range checks; conversion overflow
is an error. A representable Single outside the choice list returns NULL.
Boolean conversion overflow also remains an error. Operational failures stop immediately, including when a cancellation
callback returns true only once. The state machine latches these failures even
if a direct caller ignores its returned error; finish cannot publish success.
IIf evaluates its branches before converting its condition, as before.

Public typed ExpressionValue APIs now treat Empty as false in IIf/Switch and as
a zero/out-of-range Choose index. Selected Empty remains Empty and still fails
ordinary stored-scalar publication; it never becomes stored NULL implicitly.
Zero-length Text remains Text and fails numeric conversion under the current
profile. This follows Microsoft's [Empty coercion rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e2637723-3010-449d-bead-9f17533dc105).
It introduces no SQL Empty constructor, source/parameter carrier, VBA runtime or
IsMissing function. Missing and Error states remain invalid condition/index
inputs; explicit numeric Error consumers retain their separate contract.
SQL logical/bitwise operators and WHERE/HAVING/ON predicate coercion retain their
separately documented profiles. Full Variant compatibility is still open.

The updated batch passes 776 expression/query tests including doctests, with
296 separately run parser-free expression tests and seven selected CLI tests.
Focused controls check plan/policy reuse, numeric Date truth/index selection,
Error/Empty/Missing handling, selected payload preservation, eager RNG draws and
conversion failures. Every checkpoint before a later host callback is cancelled
with a transient one-shot flag; no later callback runs. Exact Text budget refusal
and ignored operational-error latching also pass.

Original `testing/compare_selector_coercion.py` passes 1,072 independent Decimal,
selector, DATE, eager-failure and native-field/domain controls. Inert Jackcess
getters supply original ScalarSamples IDs/timestamps on V2000/V2007/V2016/V2019;
all original source hashes remain unchanged. Native Access query results, locale,
return-type promotion and error-order qualification remain pending. All-feature
and minimal Clippy, warnings-denied API docs, formatting and diff checks pass.

## Query result text-policy lineage

Bound Switch/Choose outputs now retain static policies from possible result values,
using the same conservative merge rule as IIf. Switch conditions and the Choose
index do not contribute to the output policy; their own evaluations still use
normal comparison/conversion rules. Equal result identities remain one identity;
distinct identities remain Conflict even when a branch is unselected. Literals
have neutral provenance. The walker does not evaluate selector arguments to guess
which branch will run.

Metadata preparation remains inert. Raw projection of a conflict or unknown
identity remains possible; an operation requiring text comparison resolves or
refuses it explicitly. Derived/named aliases, predicates, groups, sort and set
keys retain the output lineage. Explicit global comparison overrides remain
unchanged. Eager errors, shared limits, cancellation and binding depth refusal
remain in force. See [the query contract](../query/QUERY_EXECUTION.md#bound-expression-text-policy-lineage).

Microsoft's Access [Switch](https://support.microsoft.com/en-us/access/switch-function)
and [Choose](https://support.microsoft.com/en-us/access/functions/choose-function)
references document selection and query usage. They do not specify mixed native
field-policy precedence; this is the existing provisional provenance model
extended consistently to these selectors, not native Access qualification.


## Single precision of Choose indexes

The [formal Choose signature](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/2f178971-8a8b-4d07-a21e-d0fa82b9db7c)
specifies Single index admission and Integer selection. The
[Access query examples and rounding contract](https://support.microsoft.com/en-us/access/functions/choose-function)
establish direct SQL scope, eager choices, rounding and out-of-list NULL. The
implementation now uses checked Single conversion before integral rounding; a
selected index must also fit Integer. This is an explicit specification model,
not native Access query-wrapper qualification.

For example, a Double/Decimal/Text index 1.49999999 rounds to Single 1.5, then
Integer 2; 2.50000001 also becomes Single 2.5 and selects choice two. The earlier
direct LongLong conversion selected choices one and three respectively. Legacy
DATE serials follow the same argument conversion. Choice values are neither
converted nor reformatted. SQL argument-count limits remain separate.

The correction also removes the earlier overflowing-Text-to-NULL exception.
A finite 1e30 index is representable as Single and returns NULL outside the list;
1e100 or numeric Text 1e1000 reports Overflow. Conversion failures remain latched
until finish, so a later failing choice expression remains observable. Existing
resource/cancellation failures still abort immediately.

Tests cover near-half boundaries, Decimal/date/Single sources, parameter-plan
reuse, explicit-policy Text, extreme range failures, selected Error payloads and
eager unselected failures. The independent runner uses IEEE binary32 packing
before Python Decimal half-even rounding, and checks ordinary/selected-index
queries over unchanged registered native rows. Query-wrapper precision, native
NULL/error precedence and full Variant promotion remain open.


## Literal selection and result metadata

Preparation now retains the selected branch's complete output descriptor when
closed scalar literal controls establish which IIf or Choose choice can return.
Switch omits known-false alternatives and alternatives after a known-true pair;
an unresolved earlier condition remains a possible choice. A guaranteed true
tail removes the extra no-match NULL possibility, while a selected nullable
branch or selected NULL retains its own nullability. Unknown controls keep the
existing conservative merge. Nz's existing nonnullable-input inference is
unchanged.

For example, `IIf(True,A,B)` preserves A's Decimal precision/scale even if B has
different constraints or a different carrier. `Switch(Flag,A,True,B)` still
combines both reachable choices, but cannot introduce NULL solely from a missing
match. Literal Choose indexes reuse the existing Single conversion, half-even
rounding and one-based range policy: 1.49999999 selects choice two, while an
out-of-list representable index yields NULL metadata. Invalid controls such as
1e100 remain runtime failures; preparation does not raise or hide that failure.

This analysis reads only captured scalar literals, including checked Number and
Date literals. Fields, parameters, Text controls, casts and other calls remain
unknown. It never executes a provider, scans a record or evaluates a general
expression. Number spellings were already validated and charged during binding;
re-reading a closed literal retains their 128-byte ceiling. Selector analysis
checks the parent binding budget/cancellation before literal processing and
meters the bounded Choose payload-index walk. No payload vector is allocated.

Every choice still binds and its result metadata is visited. Every control and
choice still executes in written order, including a known unused branch, later
Switch controls and out-of-list Choose calls. A precise descriptor describes
successful returned values, not a promise that a call will succeed. Division,
clock/provider, constructor and publication failures remain observable. Raw
String payloads keep their existing observation/publication boundaries; selected
values are not converted to fit the metadata. IIf/Switch/Choose retain their
Variant declaration categories independently of the narrower carrier descriptor.
Text-policy lineage still includes all applicable alternatives. The refined
descriptors can participate in existing derived/named and UNION result rules;
they do not introduce a new selector coercion policy.

The [Microsoft Access IIf contract](https://support.microsoft.com/en-us/access/functions/iif-function),
[Switch query contract](https://support.microsoft.com/en-us/access/functions/switch-function)
and [Choose query contract](https://support.microsoft.com/en-us/access/functions/choose-function)
establish branch selection and eager evaluation. The metadata refinement applies
those contracts to TableCraft's existing result carriers; it does not qualify
native mixed-type return promotion. Tests cover exact Text/Decimal constraints,
NULL, near-half Single index rounding, first-match selection, unknown controls,
derived/grouped/empty results, Variant widening, raw String observation, unused
clock calls and eager failures without preparation scans.
