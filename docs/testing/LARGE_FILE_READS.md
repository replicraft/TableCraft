# Larger native read-source qualification

Qualified Jet 4 and ACE 2007/2016/2019 readers accept page-aligned read sources
up to 2 GiB. Reads seek to requested pages rather than retaining the complete
source. NativeImage and OfflineReadTransaction keep a separate 64 MiB image cap,
checked before image allocation. Existing row, map, page-read, payload, cumulative
work and output limits remain unchanged. This is a larger source envelope, not
unrestricted large-table or native Access compatibility.

Microsoft's [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
list a 2 GB database limit. The explicit TableCraft envelope does not establish
support for every file below that size or validate unknown format variants.

## Synthetic bounds and host tests

The engine large_files integration suite uses an original sparse Read+Seek source:
a small known prefix and one data page at byte offset 2 GiB minus 4,096. No 2 GiB
buffer or disk file is allocated. All four current read profiles retrieve the
second row from this final page under a 1 MiB resource budget. Physical reads stay
at most 4,096 bytes each and below 128 KiB cumulatively. Allocation/read accounting
leaves more than half the byte allowance unused. These are logical budget/read
bounds, not an operating-system peak-memory measurement.

Tests also reject oversized/unaligned files, bad final pages, references past EOF,
exhausted budgets and cancellation. Complete-image capture rejects sources just
above 64 MiB and at 2 GiB with only a 66-byte prefix read, even with an unlimited
byte allowance. No bulk allocation/read occurs. A filesystem transaction test
checks the same image cap and verifies the OS lock is released on rejection.

The CLI browsing/large suite uses original synthetic 128 MiB sparse files for
all four profiles, with a data page at the end. tables --count, inspect --table,
rows --offset/--limit and bounded SELECT succeed. File length, original prefix
and final data page remain unchanged. An oversized Jet 4 update fails with no
output copy. Sparse allocation characteristics depend on the host filesystem;
these tests do not require a 2 GiB physical fixture or native Access runtime.

## Independent padded-input comparisons

The existing reviewed ScalarFixture inputs can be copied and zero-extended to
128 MiB without changing their original prefix. These are padded engineering
derivatives, not authentic large Access-authored databases; appended space is not
qualified as a correct native global allocation state. The independent Jackcess
reader checks existing catalog/schema/rows, while the CLI runs the existing
browsing/query regression controls. High-offset user records are covered by the
separate synthetic cases above, not by these padded derivatives.

Create a new local directory, preserving the original inputs:

```python
from pathlib import Path
import shutil
source = Path("/path/to/original-scalar-corpus")
target = Path("/tmp/tablecraft-large-scalar-copies")
target.mkdir(exist_ok=False)
for original in sorted(source.glob("Scalar-*")):
    if original.suffix not in (".mdb", ".accdb"):
        continue
    if original.stat().st_size > 128 * 1024 * 1024:
        raise ValueError("Expected the original small ScalarFixture corpus")
    with original.open("rb") as reader, (target / original.name).open("xb") as writer:
        shutil.copyfileobj(reader, writer, 64 * 1024)
        writer.truncate(128 * 1024 * 1024)
```

```sh
CARGO_INCREMENTAL=0 cargo build --workspace --release --no-default-features --locked --offline
python3 docs/testing/compare_browsing.py --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft --directory /tmp/tablecraft-large-scalar-copies
```

Original recipes, reviewed artifact/license provenance, source and derivative
hashes and observed results are recorded in [FIXTURES.md](../FIXTURES.md#larger-read-source-derivatives).
Neither the independent reader nor these synthetic checks establish native
Microsoft Access, encrypted-file, shared-file or large-table conformance.

## Remaining scale work

Bounded row paging now validates maps upfront and uses an incremental address
cursor without the 4,096-address vector cap. [Large-table evidence](ALLOCATION_MAP_PAGING.md)
covers 6,000-page tables. Catalog/full snapshot/index/write consumers retain their
bounded materialized maps. The existing 4,096 physical-page-read ceiling and
byte/work allowances still constrain requests. Resumable cursor continuation
remains unimplemented; repeated deep offsets still rescan earlier records. Query snapshots remain at most 16,384 source rows and results are buffered;
streaming, TOP pushdown and scalable joins/grouping remain future work. Complete
image/edit paths remain limited to 64 MiB. Use stable/offline copies throughout.
