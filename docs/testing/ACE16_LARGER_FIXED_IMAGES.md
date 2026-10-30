# Larger fixed-update image capture

The independently authored larger-image SOURCE corpus passes inert qualification:
12 original/complete expected pairs (24 images) and two additional over-cap
originals. It covers both ACE 16 header profiles, 64 MiB, the first page beyond
64 MiB, 80 MiB, and a preserved Boolean source four pages beyond 64 MiB. This is
inert SOURCE evidence. Default `NativeImage::capture` retains its 64 MiB ceiling.
Explicit `capture_for_fixed_updates` now accepts validated positive limits up to
80 MiB and retains that policy privately. It permits immutable capture and inert
inspection only; every mutation and normal plan remains denied under that policy,
even on a small file or an empty/no-op request. No larger writer is implemented.

Indexed fixed sources contain three rows and two aliased physical integral
indexes. High-address variants relocate DATA, INDEX roots and their exact ROW
references; the 80 MiB high variant also places its INDEX-map container and GLOBAL
bitmap above 64 MiB. Every retained page, including opaque tail pages, has explicit
allocated coverage through a complete indirect GLOBAL map. Preserved Boolean
sources reuse the existing independent sixteen-field model with calculated,
complex and LVAL siblings. The expected change remains one Boolean bit; its
unrelated values and stored bytes stay intact.

The qualifier checks header/catalog/schema, original rows, scalar index and
constraint coverage, exact page/map/ROW identities, preserved Boolean projections,
and complete byte differences. Moved old DATA and INDEX bodies remain opaque
originals. Every abandoned page and every byte outside the selected differences
is checked for equality. Cases are constructed sequentially to bound retained
memory. The default capture API continues to refuse every above-64-MiB case.
Explicit capture uses the same complete-byte retention, stable/offline source
contract, checked EOF/header/catalog and cumulative resource/cancellation controls.
The API is available in the standalone engine without the preview feature; caller
limits do not change allocation, encryption, journal or normal-save ceilings.

The first compile failed because the test treated `IndexInspection` as its index
collection. The correction used its actual accessor. The first runtime then
found that the initial expected constructor retained a change in abandoned low
DATA. A separate named expected derivative restores abandoned DATA and INDEX
bodies from the immutable original source. The original constructor and original
source bytes remain unchanged and callable; the derivative qualified before any
new capture policy or writer existed. These failures and the passing rerun are
preserved in the [execution record](results/ace16-larger-fixed-image-sources-2026-10-30.json).

Actual extended capture now retains all 24 images byte-exactly. Existing source
owners certify 20; four header-5 rich sources explicitly refuse with
`Unsupported(Column)`. Their native fields 7/8 are 42-byte Extended Date/Time,
whose scalar contract requires the ACE 2019 profile. The first capture-source
check incorrectly expected those to qualify. Only its expectation changed;
original source and expected bytes stayed frozen. Boolean-only inert projection
in the earlier SOURCE gate did not establish full preserved-source admission.

All 30 public mutation/preparation entries refuse the extended policy on small
Jet 4 and both ACE 16 header profiles. Limits tests pass for exact/one-below
Bytes, Rows and Steps, exact source-size ceilings, and the final observed cancel
poll. Normal and standalone minimal checks pass. The capture-only regression
round passes 541 native mutation, 47 preservation and 37 fixture-consumer
functions, with two export helpers ignored. All-target/all-feature native strict
checks, native formatting, documentation links and dependency boundaries pass. See its
[capture execution record](results/ace16-larger-fixed-image-capture-2026-10-30.json).
The earlier SOURCE strict/format/link/boundary results remain recorded separately.

This grants no larger fixed UPDATE execution, allocation growth, native Access
save, macro execution or journal admission. A later separately reviewed fixed
writer must retain the original capture policy through independent candidate
recapture while every other operation remains denied.
