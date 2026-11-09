# Fitted ASCII field rename observations

The [9 November 2026 record](../../results/schema/fitted-field-rename-raw-sdk-read-2026-11-09.json)
freezes eight independently modeled grow/shrink images over the same four pinned,
empty ACE16 explicit-OFF sources used by the earlier same-width experiment.
Number and compressed Short Text targets include retained property histories.
The requested names are `TaxAmount` and `X`; all originals contain five-unit
ASCII names. No source profile was expanded by this experiment.

Four baseline and 32 variant observations used the retained Jackcess 5.0.4
metadata-only reader, with expressions disabled. All eight coordinated images
open and retain metadata, field properties and database OFF settings except for
the requested name. Every original and observed file retains its hash.

| Variant | Images | Opens | Matching metadata except name |
| --- | ---: | ---: | ---: |
| Coordinated grow/shrink | 8 | 8 | 8 |
| TDEF-only rename | 8 | 8 | 0 |
| Stale descriptor length, grow | 4 | 0 | 0 |
| Stale descriptor length, shrink | 4 | 4 | 4 |
| Stale live-user ROW offset | 8 | 0 | 0 |

**Reader blind spot:** all four stale-descriptor shrink controls pass these SDK
getters. Reader success therefore cannot establish descriptor/ROW length agreement
or native writer correctness. The native implementation must check those lengths
and full image preservation independently. TDEF-only controls lose the expected
field-property lookup; failed controls remain recorded rather than excluded.

The raw model keeps the file at eleven 4096-byte pages and retains all row slots.
It relocates the live user body and following OFF payload within their existing
LVAL page, updates the actual descriptor length, and adjusts the TDEF name table
and framing. Retired history bodies, field identities, property values, allocation
maps and DATA/INDEX pages remain intact. Newly freed bytes retain their originals.
The record includes complete image SHA-256 pins, tool/JAR pins, exact framing facts
and path-sanitized command templates; no binary or private database content is
published. Existing same-width fixtures and observations remain separate.

This is first-party raw construction plus independent metadata observation:
**zero SDK rename writes and no Microsoft Access execution or saves**. At this
record's freeze, Rust fixture golden checks and native fitted-writer tests had not
run. Their later validation must be recorded separately; these observations grant
no journal, runtime or GUI save authority and claim no Windows/Access parity.
