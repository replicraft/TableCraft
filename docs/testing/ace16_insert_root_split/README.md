# Saved ACE 16 INSERT root-split evidence

Run the saved-only source/reference proof from the repository root:

```sh
python3 docs/testing/ace16_insert_root_split/verify.py --verify-git
```

The verifier reads the private artifacts named by the frozen protocol. It uses
`scripts.compat_artifacts.ArtifactStore` directly for root-confined fresh pins
and JSON reads. It starts no compiler, native producer, JVM or fixture process.
The optional `--verify-git` check uses one local `git cat-file --batch` process
for 749 historical compiler-source blobs and the committed build receipt. Without
that option, the output explicitly reports historical Git verification as unrun.
Missing private artifacts or requested historical Git objects produce `available: false`,
`verified: false` and exit 2; an available but mismatching artifact fails with
exit 1. A checkout without the private corpus is not passing compatibility
evidence.

This checkpoint verifies four admitted sources and the actual first 25 Jackcess
source/reference commands: one compilation, then four source preparations,
eight before getter/map calls, four explicit INSERT calls and eight after
getter/map calls. Their exact order and argv come from the frozen protocol.
The saved report retains its last attempted label in `active_command` after
completion; terminal command statuses and the completed report establish the
recorded outcome. No command is repeated by this tool.

The sources contain 451 distinct Int32 primary keys, spanning negative and
positive values, with either fitted NULL/empty Binary records or 84-byte Binary
records. Their original primary roots have height zero, prefix one, 3,609 active
bytes and seven free bytes. Inserting ID 226 produces 452 entries, whose required
3,617 bytes overflow the 3,616-byte leaf capacity. All four public references
retain the original root as a height-one node and add two 226-entry leaves.
The full-record cases also allocate one visible DATA page first.

`model.py` is the frozen independent source/request-only engineering model from
`.cache/ace16-insert-root-split-independent-next-20261027/model.py` (SHA-256
`1f98c274b406b7b3602e28647210a24b64f5e49f2eda36bd2b8116dafd4b3a75`).
`reference.py` maintains the executed source/reference raw proof. Each differs
from its frozen private source only in the repository-root bootstrap, changing
`parents[2]` to `parents[3]` for this public location. The verifier checks both
identities and the complete one-line transformation. Neither model imports the
Rust insertion planner, serializer, composer or auditor.

All four complete engineering images are derived from original bytes, metadata,
ROWs and requests before any public after-image is inspected. The expected
policy allocates DATA first when required, then the RIGHT and LEFT index leaves,
retains the old root's unused residue, and initializes fresh leaf padding to
zero. Both leaves have prefix zero, 2,034 active bytes and 1,582 free bytes.
The root stores the complete left maximum key plus ROW and left child, with the
right child as tail. Table and historical primary counters each increase once.

The verifier rehashes all 145 frozen inputs, exact command streams and report
artifacts, then rechecks each distinct identity at completion. It independently
reconstructs every source getter/raw-record/index/map fact and repeats the
public transition proof, including all 1,804 old ROWs and 1,808 final complete
key/ROW entries. Typed and raw records, represented owned/hint/global/index maps,
counters and full graph membership agree between expected images and public
references. All 6,276 differing bytes are unused fresh INDEX padding; their
positions are returned explicitly. Those public scratch bytes do not replace
the engineering zero-padding goldens.

The four engineering hashes and source closure were frozen before the new
committed producer build and actual candidate calls. The first producer run
matched all four complete images. Its separate first fresh-getter run completed eight calls and matched
every saved public getter/map stream, after whole-image equality was recorded
for all four candidates. `acceptance.py` rechecks those 12 exact argv, terminal
statuses, typed notices, before/after input pins and raw outputs; it also decodes
each actual candidate's complete tree, records and map transitions using the
original request-bound model. Candidate padding and retained-root residue must
match the frozen engineering image exactly.

The actual artifact is bound to compiled revision
`0739bc27f6eae90b15942e6171bd61aa1ba171e1` and receipt revision
`10262cc2732008fceaff37c3c2789ea4cdf1d4a0`, using the committed
[build receipt](../results/ace16-insert-root-split-example-build-2026-10-27.json).
The artifact SHA-256 is
`3f98b59dd5b8b602d0b79f8bb618fb60b8646874b1dcb577c5d658fa57fbccdb`.
The archived dependency-info bytes identify 737 dependency paths plus 12
manifest/lock/toolchain inputs, agreeing with the receipt's 749-source closure.
The verifier does not depend on the old disposable `target/` dependency-info
path. A requested historical Git check validates those exact source bytes and
the receipt against their historical blobs rather than inferring provenance
from a current working tree.

The initial source/reference-only verifier and its result remain separately
preserved in the private verifier history. Its first local draft expected a
cleared terminal label; the saved controller actually retains its last label.
That assertion was corrected before the reference proof passed, without changing
any executed source, raw report, request or golden. The actual candidate extension
passed its first saved-only run.

These results qualify four engineering candidates and independent Jackcess
observations. They do not establish native Microsoft Access save/reopen
compatibility, data-macro semantics, journalability or normal host INSERT support.
