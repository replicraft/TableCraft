# Captured Date and Now insertion defaults

Normal INSERT now resolves qualified stored `Date()` and `Now()` defaults from
one explicitly supplied host clock. CLI `query journal-insert` supplies its
existing HostClock automatically; optional `--clock LOCAL-DATETIME` fixes the
execution input. It is not a permission flag. Host execution uses local civil
time and whole-second precision. The initial native profile is an unindexed,
dense two-field table: manual Int32 followed by legacy Date, with only the
qualified Date/Now property. Broader schemas, indexes and generated-ID/default
composition remain work.

Original supplied/omitted presence is retained. Complete Source, declarations,
carriers, row ceilings and original capacity qualify before the provider is
called. All needed defaults bind before at most one reading, including a
multirow batch. Supplied NULL, complete values, empty requests and unneeded
defaults do not sample time. Unknown stored calls refuse; inspection never
executes default expressions. Existing library APIs without a Clock capability
keep their ambient-free behavior.

TCNA5 wraps the unchanged TCNA2 presence grammar with a bounded, canonical civil
reading or exact absence. It stores neither expanded rows nor a provider.
Checked V1 recovery validates original identity, rebinds declarations and
independently derives the complete image from that retained input. Active,
committed and retired replay never resample time or replay SQL. Strict framing,
reserved bytes, calendar validation and the total 64 MiB ceiling remain explicit.

Eighteen source views passed native and independent inert reads before matching
writers. Fourteen inserted outcomes and four empty cases, literal intent wires,
independent proof/forgery, complete resource/cancellation and replay controls
pass. Twenty-four retained typed/SQL saves, sixteen cold host recovery contexts
and twelve CLI saves/readbacks also pass. A copied codec test had paired an old
frozen digest with the wrong BEFORE input; the input was corrected while all
expected wire bytes were preserved. See the
[execution record](../../testing/results/ace16-normal-clock-default-writes-2026-11-01.json)
and [source qualification](../../testing/ACE16_WIDE_INLINE_CLOCK_SOURCES.md).

This does not establish native Access statement-versus-row clock timing,
Windows timezone behavior or native save/reopen compatibility. Those comparisons
remain unrun; the current host policy is explicit.
