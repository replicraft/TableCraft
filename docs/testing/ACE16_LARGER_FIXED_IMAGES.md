# Larger fixed-update image sources

The independently authored larger-image SOURCE corpus passes inert qualification:
12 original/complete expected pairs (24 images) and two additional over-cap
originals. It covers both ACE 16 header profiles, 64 MiB, the first page beyond
64 MiB, 80 MiB, and a preserved Boolean source four pages beyond 64 MiB. This is
source qualification only: complete `NativeImage` capture still defaults to a
64 MiB ceiling, and no larger-image mutation capability is implemented here.

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
memory. The existing capture API continues to refuse every above-64-MiB case.

The first compile failed because the test treated `IndexInspection` as its index
collection. The correction used its actual accessor. The first runtime then
found that the initial expected constructor retained a change in abandoned low
DATA. A separate named expected derivative restores abandoned DATA and INDEX
bodies from the immutable original source. The original constructor and original
source bytes remain unchanged and callable; the derivative qualified before any
new capture policy or writer existed. These failures and the passing rerun are
preserved in the [execution record](results/ace16-larger-fixed-image-sources-2026-10-30.json).

Native strict checks for the library and this target pass. Formatting, local
documentation links and dependency boundaries pass. This evidence does not
qualify larger fixed UPDATE execution, allocation growth, native Access saves,
macro execution, encryption changes or journal admission. A separately reviewed
read-only capture policy must qualify actual source ownership before any larger
fixed writer permission is introduced.
