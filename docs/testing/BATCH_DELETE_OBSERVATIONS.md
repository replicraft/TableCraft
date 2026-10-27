# Original multi-table DELETE batch observations

The [28-control receipt](results/ace16-batch-delete-originals-2026-10-26.json)
records 16 successful explicit candidates and 12 typed refusals over two fresh
repository-owned ACE16 originals. Both group orders produce identical candidate
bytes for every positive selection. This qualifies an engineering final-image
batch against public Jackcess getters and independent raw models. It does not
establish Microsoft Access SQL action timing, data-macro eligibility or normal
save/reopen compatibility.

## Originals and actual geometry

[BatchDeleteFixture.java](BatchDeleteFixture.java) uses the reviewed Jackcess
5.0.4 JAR and fresh-output-only public builders. Each original has 500 `Parents`
rows and 500 matching `Children` rows, non-NULL Int32 ID/ParentID relationship
keys, and a known enforced RESTRICT declaration. Each owner also has ten
independently reproducible GUID fields and unique `Wide`/`WideAlias` declarations
sharing real physical storage. Parent/child insertions alternate. A separate
`Sentinel` row holds Unicode Memo and 12 KiB OLE bytes.

Expressions are disabled, links throw, and original column/table properties are
absent. No AutoNumber, calculated field, imported binary fixture or stored active
application code was introduced. The original files remain immutable. Exact
original/reference hashes and generation commands are registered in
[the fixture ledger](FIXTURES.md#original-multi-table-deletion-batch-inputs).

| Profile | Owner definition | Wide root | Height | Owned pages | Leaves |
| --- | ---: | ---: | ---: | ---: | ---: |
| V2016 Parents | 89 | 92 | 2 | 42 | 38 |
| V2016 Children | 93 | 97 | 2 | 44 | 40 |
| V2019 Parents | 106 | 109 | 2 | 42 | 38 |
| V2019 Children | 110 | 114 | 2 | 44 | 40 |

Public getters and independent raw decoding establish full key/row maxima,
original leaf partitions, per-height reciprocal sibling chains and exact owned
map membership. The parent primary/reciprocal declarations share a physical root;
the child ParentAsc/ParentAlias/foreign declarations also share one. The two wide
owners have nonroot pages in 15 shared global bitmap bytes for V2016 and 14 for
V2019. Height and shared-byte claims come from actual originals, not target sizes.

## Reference actions and explicit candidates

Four selections are derived before mutation from original keys and locations:
full subtree-boundary maxima, the union of one original child subtree from each
wide owner, all but ID 1, and all rows. The same selected IDs identify matching
parents and children. Public reference copies delete children first, then parents,
through successive public `deleteRow` calls with referential enforcement enabled.
All eight reference copies reopen successfully.

The frozen `preview_ace16_batch_delete` tool uses committed revision
`eae88945eb59b3cb84e1fc2d6c1b3327e356c839`, SHA-256
`e451a6ca7a41df6e226831f41f1111f42741dd2f0803749427860cca653692d9`.
Its [build receipt](results/ace16-batch-delete-example-build-2026-10-26.json)
pins 42 compiled Git objects. The replay rehashes those historical objects rather
than the concurrently changing engine checkout.

| Selection per owner | Selected rows | Parent pages retired | Child pages retired |
| --- | ---: | ---: | ---: |
| Boundary maxima | 2 | 0 | 0 |
| Two original subtrees | 349 | 19 | 18 |
| Retain one | 499 | 39 | 41 |
| Delete all | 500 | 41 | 43 |

Both group orders are checked for each selection/profile. The subtree selection
changes bits for both owners in 10 shared global bytes for V2016 and 8 for V2019;
the larger selections exercise all 15/14 shared bytes. Exact union bits, separate
owned populations, root identities, retained heights, child sequences and full
separator maxima pass. Accepted tail or bounded-final-child forms are checked
against independently derived children; reference and candidate files are not
claimed byte-identical.

All 1,000 original record bodies, including deleted records, and the entire
sentinel/long-value storage remain unchanged. Every changed byte is checked
against exact count, tombstone, index encoding/link and bitmap spans. Retired
pages retain all bytes after their four-byte invalidation prefix. Existing
historical index counters and declaration identities remain exact.

The twelve negative controls require the expected error class and empty stdout:
parent-only and surviving-child requests report `MissingParent`; stale and
repeated raw requests report `Conflict`; repeated table groups refuse at input
admission; existing outputs report `AlreadyExists` and retain their bytes.
Failed candidates create no new output. This non-NULL relationship corpus does
not resolve the separate public-reader NULL-parent action discrepancy.

## Reproduction and retained evidence

[The runner](compare_batch_deletes.py) requires explicit frozen executable/hash/
revision, the reviewed JAR, unchanged registered originals, and fresh work/report
paths. Request files contain actual snapshot-local ordinals and independently
extracted original raw record bytes. [The model](batch_delete_model.py) validates
authored GUIDs, complete values/index orders and raw topology without executing
SQL or using candidate results to choose expectations.

```sh
python3 docs/testing/compare_batch_deletes.py \
  --example .cache/tablecraft-bin/ace16-batch-delete-20261026 \
  --example-sha256 e451a6ca7a41df6e226831f41f1111f42741dd2f0803749427860cca653692d9 \
  --compiled-revision eae88945eb59b3cb84e1fc2d6c1b3327e356c839 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --workdir .cache/batch-delete-independent-recheck-20261026 \
  --report .cache/batch-delete-independent-recheck-20261026.json
```

The accepted run completed 88 Java/candidate commands, with no disagreements.
Sixteen immutable input/tool pins, 56 request files, eight reference files,
16 candidates and all 42 compiled-source objects were independently rehashed.
The producer and runner remain byte-identical to the executed replay sources.
The public model removes one extra EOF blank line after exact executed bytes were
archived. Its AST is unchanged; the receipt maps historical source pins to that
inert archive. The accepted replay and all raw reports are unchanged.

Expanded rows, keys, partitions, stdout and map arrays remain ignored. The compact
receipt pins the unchanged 217 KB accepted summary, 6.53 MB full report, 6.29 MB
command JSONL and 12.52 MB snapshots. It retains both command associations and
candidate digests for every order pair, with one structural proof per identical
byte result. An initial read-only geometry probe used the wrong map-result key;
its source/error log are preserved separately. No engine, selector, resource
limit or expected-data correction was needed, and no replay was rerun for style.
