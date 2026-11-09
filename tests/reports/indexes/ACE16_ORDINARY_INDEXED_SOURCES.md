# Ordinary counter and retained-index sources

`canonical_ordinary_indexed` independently authors 24 source recipes from the
original canonical Int32 and short-variable envelopes. Header selectors 5 and 6
produce 48 inputs: eighteen positive read views and thirty adversarial controls.
No event payload is removed, and no normal planner, creator or SDK writer supplies
the source bytes or expected results.

The nine positive recipes cover an Int32 AutoNumber declaration without an index;
primary, ordinary UNIQUE and nullable include-NULL Id indexes; compatible logical
aliases; unchanged Text/Binary/Decimal siblings; a primary leaf relocated to page
95; a root with two linked leaves and prefix compression; and an inactive INDEX
tail filled with `A7`. Stored LAST values include 137, −7 and zero, independently
of live Id values. Physical index history 99 is retained independently of current
membership. The nullable Id keeps its inactive payload bytes unchanged.

Controls cover malformed generator declarations, orphan or incompatible index
metadata, ignore-NULL policy, shared map references, overlapping DATA/INDEX
membership, free or missing owned pages, unreachable pages, incorrect key/ROW
associations, an invalid node bound and an unexplained header byte. Their
expected rejection by a future content validator is separate from these positive
read observations.

Two native source tests pass: all 48 inputs construct, and all eighteen positive
views match their literal rows, index metadata, complete table/index checks and
constraint observations. The explicit exclusive exporter passes and saves all
48 originals and eighteen expected SDK reports.

The separate read-only observer passes compilation and all eighteen exact SDK
reports. Reports include the four database metadata operations, user schemas and
typed rows, raw records, actual stored LAST, physical index history, logical
aliases, cursor membership and complete owned INDEX pages. Saved verification
checks exact stdout, empty stderr, call identity/arguments and unchanged pinned
sources, JAR and classes. An earlier controller preflight selected the `-d`
argument slot instead of the pinned JAR; it launched neither compiler nor SDK.
That correction remains recorded in the compile preflight evidence.

These are source-read results. They grant no ordinary-content certificate, normal
write permission, index mutation or counter-generation authority. Validator,
journal/recovery and Access session outcomes require separate acceptance.
Native Microsoft Access and Windows comparisons remain unrun. See the
[source result](results/ace16-ordinary-indexed-sources-2026-10-31.json) and the
[larger inline-map sources](ACE16_ORDINARY_INLINE_SOURCES.md).
