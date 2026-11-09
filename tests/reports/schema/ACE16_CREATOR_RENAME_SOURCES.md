# Creator external-OFF rename source evidence

The [read-only SDK observations](results/ace16-creator-rename-sdk-read-2026-11-08.json)
cover eight independently authored Name-only AFTER files over the previously
[SDK-written explicit-OFF sources](ACE16_DATABASE_OPTIONS_SOURCES.md). The two
sources began as current empty keyless and generated-primary creator outputs;
explicit OFF added one ordinary MSysDb row and its single external property body.
Missing creator defaults remain distinct from that explicit operation.

The [literal model](ace16_creator_rename/model.py) authors growth, shrinkage,
non-BMP Unicode and leading-BOM spellings on both sources. It changes the user
Name and its footer, repacks the appended MSysDb record without changing its
bytes, and adjusts the two catalog directory entries and free-space count.
Every other original page, including the complete external LVAL and primary
INDEX page, remains unchanged. The model calls no native planner, production
record encoder or SDK writer.

All eight files reopened through the retained read-only Jackcess 5.0.4 getter.
It observed the exact proposed table name, original two columns and zero rows,
zero or one original index, one MSysDb role, and both exact LONG/DDL zero
settings. Source and AFTER hashes remain unchanged after inspection. The result
pins the model, existing getter, SDK and complete source/AFTER hashes. Generated
files and classes stay ignored. No new third-party source or database is copied.

This is independent read compatibility for literal images. It is not an SDK
rename/save observation or Microsoft Access interaction, save/reopen, dependency
repair, NameMap behavior or DateUpdate timing evidence. The separate native V3
planner and original-intent comparisons now pass in the
[completed focused batch](results/sql-creator-focused-2026-11-08.json): 35 engine
rename tests, including 54 complete-image pairs, nine explicit-OFF compositions
and 36 V3 recovery states. Four runtime rename tests pass, with creator sources included in positive
save/reopen coverage; six long-value regression cases pass too. This is local headless evidence.

The independent Rust `canonical_creator_rename` fixture adds keyless/manual/
generated empty and populated carriers plus safe constructor metadata: literal
DefaultValue zero, Required/AllowZeroLength and inert Description. Passing tests
compare complete images and preserved property bytes, then exercise refusal of
programs/references, wrong property carriers, unknown fields, malformed external
ownership and nonzero settings. They also compare the explicit option candidate
with the independently authored source before invoking rename, so the intended
composition is checked rather than inferred from a source label.

V3 is a distinct source-bound operation: user catalog slot 7, appended MSysDb
slot 8, exact catalog LvProp map cells 16/17 and one exclusive EOF body. Earlier
TCRN0001 and TCRN0002 inline source languages remain frozen. The new operation
neither silently switches settings nor changes creator defaults. Admission of
multiple user tables, subsequent add-table composition, unqualified property
programs and broader Access versions requires separate evidence and contracts.
