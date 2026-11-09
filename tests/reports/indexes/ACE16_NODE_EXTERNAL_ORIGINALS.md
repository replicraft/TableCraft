# Original sources for retained trees and external long writes

Current native read/Source checks passed on two existing independent SDK source
families before constructing new UPDATE expectations or invoking new writers.

| Family | Observed current source scope |
| --- | --- |
| Int64/Currency retained trees | Four originals, 1,600 rows, eight wider NODE roots and sixteen leaf pages; complete owned trees, all aliases, full fences, links and original histories |
| External Memo/OLE | Four raw/compressed originals, six rows each, four checked index aliases, complete LVAL inventories including existing orphan chunks, constraints and current ordinary Source/fixed capability |

The retained-tree gate establishes current readable Source admission, not NODE
mutation permission. The external gate likewise does not authorize replacing,
retiring or allocating chunks. Existing orphan bytes and shared container pages
remain original facts; they are not free placement authority.

Both gates verify immutable originals before native reads and call no matching
planner, serializer, preview or AFTER constructor. Their tests are ignored by
default and require explicitly selected source directories. No private planning
folder or machine path is required by ordinary builds or CI.

[The execution record](results/ace16-node-external-original-source-gates-2026-11-01.json)
contains exact commands, source checkpoint hashes and log hashes. Strict engine
lint passed. These checks do not establish native Access save/reopen, a future
writer's acceptance or byte identity with another engine's UPDATE algorithm.
