# Indexed BeforeChange source corpus

This corpus supplies independently authored indexed originals and complete modeled
outcomes before the normal indexed writer runs. It does not grant normal write
admission or establish Microsoft Access event execution/save compatibility.

The fixture owner is `tablecraft_testkit::access::canonical_indexed_before_change`.
It retains the existing canonical BeforeChange envelope, native MR2 declarations
and three ordinary Int32 rows: `Items(Id, Score)` contains `[1,10]`, `[2,20]` and
`[3,30]`. Each original independently adds one physical index, owned by table
definition page 7, with root leaf 9 and a new map record 3:16. Existing map records,
DATA ownership, catalog long-value roles and GLOBAL storage retain their meaning.
No database template, production index serializer or observed candidate supplies
these bytes.

Nine recipes under each header selector 5 and 6 provide eighteen originals and
sixteen complete expected images, each 40,960 bytes:

| Recipe | Index and modeled result |
| --- | --- |
| Ascending | Score ASC; original request 10→17 plus Current/Old program yields 28 |
| Descending | Score DESC; same values with reversed physical key order |
| Composite | Score ASC, Id DESC; complete two-component keys |
| Alias | Two logical names share one physical Score index |
| Unique swap | Two user requests produce final Scores 20/10 atomically |
| Macro-only target | User changes Score; program changes indexed Id to 11 |
| Restore original | User requests 17; program restores Old 10; complete image unchanged |
| Empty | No user requests; complete image unchanged |
| Unique conflict | Final Score 20 duplicates another key; no expected candidate |

Expected images begin with the original source and overlay literal final DATA
values plus independently encoded and sorted signed Int32 keys and original row
references. They retain topology, counters, prefix zero and unused leaf bytes.
The two-component case exercises the existing fitting repack geometry without
claiming nonzero-prefix or interior-node coverage. Equal-assignment admission
remains governed by the separate normal-write contract.

The source-only native gate checks all thirty-four views through scalar reads,
full table/index checks, each named index scan, exact map roles, constraints,
macro carriers and complete native program compilation with empty intents. It
executes no event row and obtains no normal mutation plan. The explicit exporter
uses fresh create-exclusive files and retains the original eighteen request
records. Ordinary tests pin all thirty-four image hashes and a separate original
request binding before indexed writer execution.

The [source execution record](results/ace16-indexed-before-change-sources-2026-10-31.json)
records native checks and the separate independent reader phase. Generated files,
compiled SDK tooling and raw observations remain ignored local evidence. These
sources cover one-leaf Int32 trees; native Access behavior, deeper topology,
nonzero prefixes, allocation, relationships and broader normal writes retain
separate acceptance work.
