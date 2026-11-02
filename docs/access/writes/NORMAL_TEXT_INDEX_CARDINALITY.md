# Fitting indexed Short Text INSERT and DELETE

Normal complete-row INSERT and original-witness DELETE now maintain qualified
nonunique General Legacy/2010 Short Text root leaves. They use the existing
native intents, recovery, retained typed/SQL sessions and CLI commands.

This extends the [indexed UPDATE profile](NORMAL_TEXT_INDEX_UPDATE.md) with
separate action facts and ownership checks. Required/unique Text indexes,
nonzero prefixes, NODEs, splits, allocation and Text page retirement remain
outside this fitting profile. Each physical alias is maintained once; complete
Source, native constraints, program absence, DATA/INDEX/map ownership and LVAL
protection remain mandatory.

INSERT uses original owned capacity and request-aligned DATA references. It
updates physical distinct-key histories only for newly distinct native keys.
DELETE removes exact original ROW references and preserves historical counters,
including when all live rows are removed. Both retain inactive page tails and
all unrelated bytes. The independent proof reconstructs the original population,
native key framing and final full image without staged keys or patches. The
shared key proof lives in its neutral native index audit owner; UPDATE imports
that owner directly.

The original source family has no stored AllowZeroLength property. Four hypothetical
empty-string INSERT outcomes remain source evidence only: their actual requests
refuse with ConstraintCoverage. A readable existing empty value does not supply
permission to insert another one. Separate [explicit true/false property
sources](EXPLICIT_TEXT_AZL.md) now pass native/session/CLI acceptance without
widening those absent-property requests. NULL insertion and deletion of existing empty values retain
their separately checked semantics.

Native engine and host checks do not establish Microsoft Access save/reopen
qualification, which remains unrun.

The [execution record](../../testing/results/ace16-normal-text-cardinality-writes-2026-11-01.json)
records source pins, native commits/recovery, direct proofs, retained saves and
CLI readbacks, including the initial acceptance failures and corrections.
