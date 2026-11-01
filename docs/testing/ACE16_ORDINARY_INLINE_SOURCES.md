# Larger ordinary inline-map sources

`canonical_ordinary_inline` authors twelve inputs from qualified original
Int32 source envelopes. Each image has 96 complete pages: metadata remains on
pages 0–7, user DATA moves byte-exactly from page 8 to page 95, and intervening
zero pages are explicitly GLOBAL-free. Header selectors 5 and 6 provide ten
ordinary views and fourteen adversarial views.

Positive inputs cover variable bitmap lengths, owned base 64/candidate base 88
windows selecting the same DATA page, future GLOBAL-free bits for pages 96–127,
and five-byte/empty out-of-file LVAL windows. Negative inputs cover owned or
candidate members beyond EOF, missing GLOBAL coverage, free metadata, physical
map aliases and indirect tags. No proposed validator, creator or writer supplies
the source roles or expected values.

The native source read/constraint test and explicit fresh exporter pass. All ten
ordinary views pass exact independent SDK observations using the existing
protocol. The observer's only source change is its file bound from 64 to 96 pages;
its predecessor, source inputs and literal reports remain unchanged. Fresh class
files and the reviewed JAR are pinned before calls. Root formatting changes to
the Rust source preserve both the original source snapshot and every exported
input/expected byte.

These results qualify input reads. Complete content validation, ordinary mutation
proof, normal journal/recovery and retained session/CLI saves have their own
acceptance checks. Native Microsoft Access and Windows qualification remain
unrun. See the [source record](results/ace16-ordinary-inline-sources-2026-10-31.json).
