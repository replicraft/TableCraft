# Typed fixed-scalar BeforeChange sources

This repository-authored corpus freezes original Boolean, Int64, Currency and
legacy DateTime requests before normal typed writer execution. Native macro
operands and SetField targets remain Int32. Source construction consumes only
the earlier mixed-schema fixture's original image, original rows and native
program, never its expected image or observed writer output.

Nineteen recipes under each header selector 5 and 6 give 38 originals and 28
literal expected images: 66 read views and 2,433,024 image bytes. Fourteen recipes
per header have a positive expected image; five retain refusal-only original
requests without inventing a failed AFTER image.

The positive cases cover Boolean changes, Int64 minimum, scaled Currency extrema,
signed-zero DATE changes, positive/negative fractional DATE aliases, NULL mask
activation and clearing, unchanged inactive DATE bytes, shared mask bytes,
disjoint rows, macro restoration beside a wide change and genuinely empty
groups. Literal expected images overlay original payloads and masks. The stored
sequential program independently implies macro-only Int32Value 1; the restore
program implies its original zero. Groups share one original image and each
event row appears once. The positive outcomes contain 28 changed roots in total.

Refusal contexts retain equal DATE assignments, NULL-to-NULL beside an inactive
NaN DATE payload, a wrong same-width carrier, a stale later expected value and
duplicate column groups. Reader acceptance does not establish the future normal
refusal, event timing or native Access behavior.

Two native source/anchor tests pass; the exporter is ignored in ordinary runs.
The source gate checks complete scalar reads, index/constraint metadata, exact
native property binding, actual preserved-source capture, raw field/mask spans
and the whole expected-image complement. It does not evaluate events or plan
writes. Image and original typed-request aggregate hashes are frozen before
producer calls. The first compile caught an exporter JSON closing-brace error;
its correction changes no model, request or expected database bytes.

All 66 independent SDK inventories pass against exact reports frozen before
calls. The unchanged reader observes all ten fields, catalog/system rows,
database metadata and raw scalar representations. Separate literal raw overlays
retain inactive NULL payloads and distinguish DATE signed zero and fractional
aliases from their calendar rendering. No Java compilation was needed. The
saved-only verifier passes without launching children. Source, image, class and
request pins remain exact; see the
[execution record](results/ace16-fixed-before-change-sources-2026-10-31.json).

These are typed ordinal witnesses. The two original SignedZero rows cannot be
uniquely selected by the supported SQL equality profile. SQL saves need separate
qualified discriminator originals or explicit both-row requests and outcomes.
Required/generated fields, late Raise, rich siblings, actual normal typed
commits and original-intent recovery remain separate acceptance work. These
fixtures grant no macro-absence certificate or write permission and establish
no native Access macro execution or save/reopen conformance.
