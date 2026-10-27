# Original-row request directory

The mutation row walker now indexes original request positions before traversing
live physical rows. This bounds request matching and duplicate detection without
changing the requested rows, callback order or candidate bytes. It adds no write
profile, field codec, normal save path or native compatibility qualification.

The implementation is in
[rows.rs](../../crates/engines/tablecraft-ace/src/table/mutation/rows.rs) and its
[private directory](../../crates/engines/tablecraft-ace/src/table/mutation/rows/directory.rs).
The [scoped record](results/ace16-row-request-directory-2026-10-26.json)
pins the source, builds, original inputs, comparisons and raw timings.

## Matching and resource contract

Each directory entry retains a live-row ordinal and its original request position.
Unordered requests sort by both fields; already ordered requests skip sorting.
The physical walker advances one cursor through this directory. Duplicate ties
retain the first original request, matching the previous linear search. The
existing final matched-request count still refuses duplicate or missing requests.
Duplicate validation separately checks adjacent ordered entries.

Empty and single-request operations retain their allocation-free paths. Every
operation still traverses the complete original live-row stream, checks page
ownership and compares the observed live count with the table declaration. The
existing selected-record layout checks, visitor failures and complete candidate
validation remain in force. Callback request references explicitly borrow the
original request slice; they never borrow a temporary sorted entry or page.

For multiple requests, retained directory memory is proportional to request count.
Construction, uniqueness checks and cursor advancement check cancellation and
charge the operation budget. Unsorted construction prepays bounded sorting work.
The operation still charges physical rows and pages independently. Directory
allocation and setup now occur before the physical traversal, so exhausted-byte,
work and cancellation boundaries can differ from the previous implementation.
This does not promise identical failure precedence under insufficient resources.

## Correctness evidence

Six private tests include an independent first-position search model over generated
request sequences, duplicate and missing ordinals, the maximum machine ordinal,
allocation/work limits, cancellation/retry, and sorted and reversed 16,384-request
walks under finite work allowances. The property test counts as one test function.
The selected existing regression batch passes 150 update/delete test functions.

The before build comes from a Git archive of committed post-projection revision
`660dae5a790c06c008be552138ed22a99cee7c18`. The contender uses the same archive
with only the three directory/lifetime source files overlaid. Both use the same
unoptimized development build flags. Unrelated slot-update work in the main
checkout is excluded from these two artifacts. All 626 recorded baseline compiler
source files match that revision's Git objects.

Each artifact reproduces 16 previously checked whole-image candidate goldens:
ACE 16 V2016/V2019 originals, boundary maxima, two original subtrees, retain-one
and delete-all, with normal and reversed request/group order. These are the same
16 controls executed before and after, rather than 32 distinct acceptance cases.
The originals contain 500 parent and 500 child rows, multiple aliased physical
indexes, a Restrict relationship and height-two index trees. Exact output bytes
match the accepted independent model in
[batch DELETE observations](BATCH_DELETE_OBSERVATIONS.md).

## End-to-end measurements

Hyperfine 2.0.0 measures the disposable engineering consumer on Darwin arm64.
Commands include TSV loading, image capture, complete checks, candidate construction,
output creation and synchronization. Each case has two command-order rounds,
two warmups and 12 measured runs per artifact per round. No Cargo process runs
during the timings. All 384 measured processes succeed; no sample is discarded.
These measurements do not establish release-build, native Access, disk-I/O or
peak-memory performance.

| Original | Selection | Before mean (ms) | After mean (ms) | Before / after |
| --- | --- | ---: | ---: | ---: |
| V2016 | Boundary maxima | 129.17 | 128.01 | 1.009 |
| V2016 | Two original subtrees | 112.74 | 111.56 | 1.011 |
| V2016 | Retain one | 96.85 | 94.64 | 1.023 |
| V2016 | Delete all | 104.15 | 96.78 | 1.076 |
| V2019 | Boundary maxima | 133.76 | 130.49 | 1.025 |
| V2019 | Two original subtrees | 110.95 | 108.43 | 1.023 |
| V2019 | Retain one | 99.72 | 98.73 | 1.010 |
| V2019 | Delete all | 99.39 | 106.58 | 0.933 |

The small differences and order-dependent results do not support a uniform
end-to-end speedup claim. V2019 delete-all includes a retained 190.56 ms contender
sample; its second round has a 1.013 before/after mean ratio. Median retired
instruction counts fall about 1.73–2.87% for the larger selections and remain
essentially unchanged for the two-row selections. The finite-work directory
controls establish the algorithmic bound separately from these modest and mixed
500-row timings.

## Preserved diagnostics

The initial main-tree test compilation stops on missing documentation in unrelated
slot-update work; no directory tests run in that attempt. The retry passes all six.
Returning from isolated archive builds to the main checkout exposes Cargo artifact
reuse with the shared target directory: the slot test initially sees the archived
API. Rebuilding the main library without changing its bytes resolves that compile
failure. Frozen artifacts, build-source pins and logs remain separate; future
isolated builds should use a separate target directory.

The first preview test lint also reports existing duplicate private fixture imports.
Its diagnostic is preserved separately from accepted verification. The coordinator
commits shared private fixture imports in `a273eb45`; the final preview library/test
strict check passes without a duplicate-module suppression. Default production
strict, the preview minimal build and scoped formatting also pass. The repair
does not change the directory, fixture bytes or expected candidate outputs.
