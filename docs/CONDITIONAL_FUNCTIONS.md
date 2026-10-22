# Eager Switch and Choose

`tablecraft-expressions::functions::conditional` owns closed Switch/Choose
identities and a materialized-value selector. Scalar evaluation and retrieval use
the same selector. No VBA/runtime/provider dependency is introduced.

Switch accepts condition/value pairs and returns the first true condition's
value. A selected NULL stays NULL even if a later pair is true. When no condition
is true the result is NULL. Conditions share IIf's explicit numeric/Boolean policy:
zero and NULL are false, nonzero exact/finite numeric values are true. Text,
date, binary and GUID truth conversion remain unsupported.

Choose accepts an index and one or more choices. Indices are one-based and use
the existing half-even numeric-to-integral rounding. A NULL or out-of-range index
returns NULL. Finite numeric magnitudes beyond Int64 also return NULL rather than
losing precision or saturating to an available choice. Invalid types/non-finite
indices fail. Returned choices retain their exact carrier and payload; no common
result type or implicit conversion is inferred.

## Evaluation and limits

Both functions evaluate **every supplied expression in written order**, including
unselected choices and later Switch conditions. Selection retains only one
chosen value and drops other evaluated payloads, without allocating or cloning an
argument-value buffer. The selector's `push` consumes values; `finish` requires
the declared argument count. Extra-argument errors are latched even if a caller
ignores the returned error. Selector state deliberately omits Debug/Serde.

Truth/index conversion errors are deferred until all arguments have been
evaluated. A later expression evaluation error therefore remains visible even
when the selector already has a conversion error or chosen value. Invalid later
Switch conditions are not hidden by an earlier match. This is the implemented
eager-error contract; native Access error precedence still needs comparisons.

Switch requires at least one complete pair; Choose requires an index and at least
one choice. Missing argument slots, unsupported qualifiers and invalid arities
fail before expression evaluation. Query binding resolves *all* operands even
when their branch would be unselected, without row scans. Grouping/aggregate
validation, parameter rebinding, correlated-slot rewriting, projection pruning,
DISTINCTROW source dependencies and crosstab expressions traverse every operand.

The selector itself does constant work per argument and owns no I/O. Callers must
supply their work limits; the scalar/query evaluators retain normal node/depth,
allocation, text and cancellation budgets. Unselected text allocations still
consume budget. Results publish only after successful complete evaluation.

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
crosstabs, errors and cancellation. The [CLI recipe](testing/compare_conditional_functions.py)
compares independent selection models and registered sequential/indexed native
rows while checking unchanged source hashes. It handles existing quoted Text
presentation through the TSV display adapter; it does not change CLI formatting.

Full Variant/date/locale coercion, native SQL return-carrier promotion, NULL index
and error-precedence qualification, and actual Windows Access comparisons remain
open. No native Access SQL execution evidence is claimed.

## Expression Error data

[CVErr Error data](VARIANT_ERRORS.md) now survives eager IIf/Nz/Switch/Choose
selection for explicit consumers. An unselected Error data value can be dropped;
an unselected failing expression still fails. IIf condition conversion waits
until its evaluated branches are available, and Switch/Choose retain their
existing deferred condition/index error policy. This does not admit implicit
numeric Error conversion, expression arrays or unconsumed Error result cells.
