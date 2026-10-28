# Saved height-one INSERT proof

```sh
python3 -B docs/testing/ace16_insert_height_one/verify.py
python3 -B docs/testing/ace16_insert_height_one/verify.py --verify-git
```

This saved-only verifier checks four fitted INSERT candidates on the unchanged
PUBLIC after-images from the earlier root-split reference. Each source has 452
rows and a height-one primary with two 226-entry leaves. Each new request adds
explicit ID 227 and either empty Binary or 84-byte Binary. These four original
requests exercise the right tail leaf. Left-branch routing has separate literal
controls; larger trees remain refused.

The actual reference phase completed its first 13 commands: one compilation,
four fresh-copy public appends and eight read-only getter/map calls. It generated
no source database. The subsequent first producer phase matched all four full
engineering images; its separate first fresh inspection phase completed eight
calls. No command is rerun by this verifier. The
[result receipt](../results/ace16-insert-height-one-native-2026-10-27.json) retains
these phases, their hashes and qualification limits separately.

`model.py` preserves the independent source/request-only model frozen before the
committed producer build and candidate calls. Its only public-location change is
the repository-root bootstrap. `reference.py` preserves the request-bound raw
proof, replacing its private dynamic parser load with a direct import from the
owning `ace16_insert_root_split.reference` module. The verifier checks those exact
transformations and the imported owner's identity. No producer serializer,
planner, composer or native auditor supplies expected bytes.

All four complete expected images are derived before reference after-images or
candidate getter streams are interpreted. Integrity pin checks can read streams
before derivation; their contents do not construct the expectations. The image
proof retains the original header, root, left leaf, sibling links and every map.
It adds a visible ordinary record to the highest existing DATA hint and one full
key/ROW entry to the existing right leaf. Both counters increase from 452 to 453.
The right leaf retains prefix zero and all original unused residue after its
2,043 active bytes. No page or bitmap is allocated, and file lengths are unchanged.
All four independently derived images also match the public references exactly.

The actual candidates preserve all 1,808 original ROWs and contain 1,812 final
leaf key/ROW pairs, plus four unchanged root bounds. Each new record is 13 or
97 bytes. New NULL requests are not part of these four cases; 452 stored NULL
and 452 stored empty Binary peers remain intact. Literal NULL insertion controls
belong to the implementation tests, not this original-file claim.

The tool uses `ArtifactStore` directly for fresh root-confined byte pins and JSON
reads. Private databases, classes, JARs, streams, frozen plans and reports are
required under `.cache/`; none are distributed here. Missing evidence produces
`available: false` and exits 2, with a portable path label. Contradictions exit 1;
a complete saved proof exits 0. A checkout without the private corpus is not a
passing compatibility result. Every checked identity is rehashed at completion.

Default verification starts no child process. The optional Git check uses one
local `git cat-file --batch` process for 751 compiled source blobs and one receipt
blob, bound to revision `96a5e91e86948923cf382dd7c682c821aee71102` and receipt revision
`647cd9cbc226711a818efc7dbf1f1b607669042c`. It verifies the exact archived dependency
information rather than requiring a disposable target directory. Neither mode
starts Java, Cargo, a native producer, a generator or a public INSERT action.

The first private actual saved proof passed unchanged. The initial maintained
conversion then exposed a local report-variable name shadowing its imported
parser module. Its source and failure are preserved; the report binding alone
was renamed, and the maintained proof then passed. No source database, request,
model, expected image, executed controller or raw report changed.

These results qualify the four engineering candidates and independent Jackcess
observations. They do not establish normal host INSERT support, journalability,
Microsoft Access save/reopen, data-macro semantics or broader INSERT compatibility.
