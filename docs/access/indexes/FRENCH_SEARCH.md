# French legacy substring search

The engine provides `find_french_legacy_text` and `find_french_legacy_utf16` for
exact French legacy descriptor `1036/0/0`. The Access adapter exposes this
provisional engineering policy through explicit Database-mode string searches.
It uses complete French weights, including reversed secondary weights, without
the physical index encoder's 255-unit prefix or digest. Other descriptors refuse;
there is no General, ordinal, Windows Text or LCID fallback.

## Consumed spans

`FrenchTextSearch::ForwardFrom` selects the earliest admitted start;
`ReverseThrough` selects the latest start whose complete span ends at or before
the supplied exclusive boundary. Both return zero-based UTF-16 ranges into the
original input. At a chosen start, the shortest matching nonempty span wins.
Equality uses the complete comparison policy, including trailing ASCII-space
equivalence. For example, a candidate `a` can match the needle `A ` without
consuming a following space.

A genuinely empty needle selects an admitted boundary. A nonempty
weight-ignorable needle must consume input. A valid surrogate pair remains one
boundary element; isolated surrogate units retain their weights. Bounds inside a
valid pair round toward the searched region. Decoded and raw entry points share
the same kernel and span policy.

The adapter advertises UTF-16 search only for Database mode. `InStr`, explicit
Database `InStrRev` and other existing span consumers retain their evaluator
argument, default-mode and publication rules. A French field under a General
operation default does not select a French string built-in automatically.

## Bounded search

French full-key encoding constructs the same append-only primary stream as
Legacy General. The French transformation reverses only the secondary stream
after primary construction. A growing candidate therefore cannot repair an
already differing primary prefix, and its primary length cannot shrink.
Trailing-space trimming can delay primary bytes but does not invalidate that
prefix property. Contextual repeat weights and primary-free modifiers retain
the same rule.

The kernel shares the existing primary-only candidate scheduler, using Legacy
per-unit primary bounds. It probes candidate lengths exponentially, then uses a
lower-bound search to avoid encoding every short prefix of a long needle. A
candidate with a differing or overlong primary prefix cannot match by extending
its end. Secondary streams never provide a pruning decision: every possible
match requires complete French-key equality.

This avoids an unconditional exhaustive-window fallback. Inputs with many
primary-free units can still require many candidates; the caller's cumulative
work and allocation budgets bound those cases. Decoding, boundary construction,
key allocations, encoding and comparison charge those same pools and check
cancellation. No per-candidate allowance is reset. The adapter shares its pool
with comparisons and descriptor resolution, and refuses reentrant mutable
callbacks.

## Independent checks and qualification

The first focused batch passes all seven kernel functions, the primary-invariant
function, six Access provider functions and seven query-contract functions.
They include literal accent/case/expansion/padding spans, raw surrogate bounds,
exhaustive short Latin windows against a separate literal equality model, and
generated raw windows against an exhaustive complete-comparison oracle that
does not reuse the production scheduler. A separate private invariant test
checks unchanged Legacy primary bytes, append-only prefixes and per-unit bounds
across modifiers, repeat characters and raw UTF-16 combinations.

Long-input controls impose fixed work/allocation ceilings on a 2,049-unit needle
and compare work counts for doubled realistic prose with an absent needle.
Resource controls include measured one-short byte/work pools, cumulative reuse
and cancellation at every observed decoded-search checkpoint.

Existing General regressions also pass: fourteen engine functions and sixteen
Access adapter functions. The full ACE library passes 601 functions with one
existing ignored generator; all 500 Access functions across 62 targets pass.
The Access run required a separate required-index ordering correction; that
defect and correction are independent of French search. Strict all-target,
all-feature Clippy and minimal builds pass for both crates. Formatting, the
document-link check and dependency boundaries for all fifteen workspace packages
also pass.
These broader totals include the focused functions and are not additional
independent test populations.

These tests establish an engineering span contract, not native Access SQL/NLS
matching. The [French comparison observations](../../testing/FRENCH_COMPARISON_OBSERVATIONS.md)
remain comparison evidence, not a native substring oracle. LIKE, character-class
matching, equality-prefix hints, additional stored sort orders and Windows
qualification remain separate work. Existing Binary operations and explicit
Text/LCID providers keep their own contracts.
