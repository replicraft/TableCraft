# Explicit constant defaults in native creation

Fresh creation authors exact Boolean, Int32, Int64, Currency, Short Text and
explicit NULL defaults through `CreateColumn::with_default_literal`. Nullable
families accept NULL; Boolean defaults require a Boolean. Values with another
carrier refuse rather than coerce. Date, clocks, generated values and arbitrary
expressions remain separate work.

The native formatter uses bounded UTF16 stack storage, explicit quote escaping
and signed integer/scaled Currency formatting without locale or floating-point
conversion. The escaped expression must fit the existing 255-unit consumer
ceiling. Overflow refuses before image allocation/publication. Initial rows still
supply complete values; defaults are metadata, not missing initial cells.

Published files use the existing omission-aware normal INSERT consumer and
independent proof. Omitted fields resolve the stored literal; supplied NULL,
false and Text remain supplied. Actual Required/AllowZeroLength constraints apply
to the resulting row. A stored default that violates those constraints does not
authorize an otherwise invalid INSERT.

The [source phase](../../testing/ACE16_CONSTANT_DEFAULT_SOURCES.md) qualified
28 complete images before creator calls. Eight single/one-table-multiple creator
contexts match complete sources; measured resource/cancellation and carrier/bound
refusals pass. Real retained typed/SQL sessions pass twenty complete saves and
readbacks, four Required/empty refusals and prepublication metadata-bound refusal.
The old Int32-default regression passes. Shared source pins/rows have one pure
testkit owner; concrete creation setup remains at each actual consumer boundary.

These are bounded engineering checks, not native Access save/reopen qualification.
Broader constant families, Date representation, generated expressions and native
application comparison remain explicit work.

The [combined execution record](../../testing/results/ace16-constant-defaults-and-node-retirement-writes-2026-11-01.json)
retains focused/strict checks and the first broad regression. One stale unit-test
error expectation was corrected and replayed; the 1,164 other engine units passed
the first run. Adapter/CLI/recovery checks passed separately.
