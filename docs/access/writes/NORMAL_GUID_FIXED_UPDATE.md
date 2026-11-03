# Ordinary unindexed GUID fixed updates

The [indexed GUID continuation](NORMAL_GUID_INDEX_UPDATE.md) shares this service
and original request dialect under separate native index/ownership checks.

The unindexed fixed UPDATE path carries exact GUID assignments through
`plan_recoverable_ordinary_fixed_updates`, retained typed Access editing and the
CLI's existing SQL GUID literals. `NativeFixedIntentGroup::new_ordinary_guid`
retains original request data without admitting a native file.

Every named target qualifies before no-op filtering, including empty groups.
Fresh complete Source and NoRequired admission remain necessary. Native type 15
and shared Guid must agree, with manual fixed flags 3, extended flags 0 and width
16. Existing overlap, dependency, constraint, indexed-target, original-value,
program and physical ownership checks remain. No GUID index rewrite, generation,
record growth or page allocation authority is introduced.

GUID values preserve all 128 canonical bits. NULL is a separate presence state;
clearing it preserves all 16 inactive payload bytes. Expected values compare
exactly, even when the replacement would otherwise be a no-op. Stage and
independent proof specialize at 16 bytes; existing entry points retain their
8-byte payload allocation geometry. Complete-image comparison and fresh final
Source protect every unchanged record, field and storage owner.

## DATA and original request compatibility

Native DATA reverses the first four canonical bytes and each of the next two-byte
groups, preserving the final eight bytes. The independent proof spells this
permutation separately from the stage encoder. Original requests retain canonical
bytes, never the native DATA permutation.

Naming a GUID target selects `TCNI0010`/profile 10, including NULL-only, no-op and
named-empty groups. It dominates Scalar/IEEE/Legacy selection regardless of group
order. Unrelated fields do not select a wider dialect.

TCNI10 retains original-group framing and complete BEFORE binding, with 17-byte
cells. GUID tag 10 contains 16 canonical bytes. Tags 0 through 9 retain their
meanings with eight additional zero padding bytes. Old TCNI3/4/8/9 keep closed
9-byte cells and target policies, including empty groups with no carrier tag.
Unknown tags, noncanonical padding and magic/profile mismatches refuse.

Old updates retain their 26-byte width, fast record-length bounds and charges;
TCNI10 uses 42-byte updates and a separate bound. The union host allowance covers
the largest record without widening old decoders. Framing, allocation, comparison
and reconstruction remain prepaid and cancellable. Cold recovery selects the
decoded profile once and reconstructs both complete images against the same
checked V1 journal before effects, without decoder or planner fallback.

## Evidence and remaining work

Two existing independent originals and four literal in-memory replacement/NULL
states establish six unindexed read/raw states, all with actual NoRequired and
canonical Source admission. Restore returns to the exact original. The separate
indexed GUID read admission does not establish GUID index mutation.

All 22 primitive functions, reader/journal/preview builds and 13 focused matching
functions passed. Native, retained typed and CLI saves each check six exact-image
outcomes. Direct proof, stale/type/presence controls, old-dialect empty-target
refusals, resource/cancellation boundaries, representative recovery and mixed
metadata orders also pass. The CLI uses existing `{GUID {...}}` literals or NULL;
no new SQL function or conversion was added.

An initial installer registration-key error prevented test installation; its
zero-test filtered runs are excluded. Correct installation preceded all matching
results. All 120 retained runtime functions passed, followed by strict all-target
checks for ACE, Access, CLI and testkit, reader/journal/preview builds, dependency
boundaries with 17 guard tests, and formatting. The
[execution record](../../testing/results/ace16-normal-guid-fixed-update-2026-11-03.json)
retains commands, log hashes and excluded preliminary runs.
The new in-memory states have no independent SDK or Microsoft Access
interpretation claim. Windows save/reopen, mixed-client, power-loss and universal
GUID write compatibility remain separate and unrun.
