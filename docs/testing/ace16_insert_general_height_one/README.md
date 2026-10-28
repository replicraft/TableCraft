# General height-one INSERT candidate observations

Four ACE 16 cases on four prepared source copies passed complete source/request-derived image comparison. Each inserts IDs **−227 and 228** into an original height-one primary with leaf populations **451 / 226 / 451**, producing five leaves of 226 entries. The cases cover V2016/V2019 headers and empty/84-byte Binary values. They preserve all 4,512 original logical ROW references and produce 4,520 final primary key/ROW pairs.

The 25-command public reference phase and four native producer calls passed on their first executions. Fresh inspection required two controllers: the original controller completed four V2016 calls, then its copied four-process ceiling stopped it before the first V2019 call. Its raw report remains unsuccessful. A separately frozen continuation ran only the four unattempted V2019 calls. All eight distinct read-only JVM calls succeeded once; no completed call was rerun. The original getter protocol declared a 16 MiB stream limit, while its implementation used a stricter 8 MiB limit. No stream hit that limit; the continuation explicitly retained 8 MiB, four processes and 30 seconds per child.

These are separate, non-journalable engineering candidates. They do not qualify normal native writes, data macros, Microsoft Access save/reopen, every field/index profile, or root growth beyond height one. See the [implemented contract](../ACE16_INSERT_GENERAL_HEIGHT_ONE.md) and [compact result](../results/ace16-insert-general-height-one-native-2026-10-28.json).

## Independent construction and saved checks

The four complete expected images were frozen from eligible **before** files, their raw records/maps and the explicit requests before the committed producer build or candidate calls. No public after-image or producer output constructed those goldens. [model.py](model.py) preserves the executed independent model exactly except for repository-root discovery. [reference.py](reference.py) similarly preserves the executed reference proof; its five function ASTs are checked. Both use their actual owning shared modules directly.

The model reconstructs complete nullable Binary records, chooses the highest DATA hint once, and derives physical ROW references. It routes by original inclusive full key/ROW bounds, completes each original child's split work before the next, assigns fresh symbols in creation order, and preserves the original root form. Existing pages retain their original prefix seed and inactive residue; fresh pages use prefix zero when fitting and zero unused bytes. The full result is independently decoded and checked against every original/requested key and ROW.

For these four actual cases, both new records fit the original highest DATA page. Each result appends exactly two INDEX pages, with no new DATA or bitmap page. Ordinary owned/candidate maps remain unchanged. The exact global-free clears and index-owned sets aggregate into nine changed map bytes across the four cases. Table and physical historical distinct-entry counters each change from 1,128 to 1,130 once per source; historical counters are not inferred to be live counts generally.

The public writer's root/leaf residue and padding differ from the engineering policy. It writes prefix zero on the retained outer leaves, while the engineering images preserve prefix one. Saved comparison finds 36,385 differing bytes across the four public after-images, all within affected index pages. Public scratch is diagnostic and never an alternate candidate golden. Both paths agree on logical keys, all original ROWs, inserted values, maps and counters.

## Reproduce the saved proof

From the repository root:

```sh
python3 -B docs/testing/ace16_insert_general_height_one/verify.py
python3 -B docs/testing/ace16_insert_general_height_one/verify.py --verify-git
```

The default verifier launches no child process. It derives and compares all four complete expected images before interpreting cached fresh getter streams, verifies the original 25 reference calls, preserves the failed getter controller report, and proves the successful four-call prefix plus four-call continuation equals the original eight-command inventory. All original inputs, candidate/stream artifacts and terminal pins are rehashed. The first full saved proof, maintained default proof and optional Git proof passed; the latter checks 756 compiled source blobs plus one committed build receipt in a single Git metadata child. These checks make zero native/JVM fixture calls.

Required ignored cache evidence is not distributed with the public helpers. Missing evidence returns JSON with `available: false` and exit code **2**; it is not an accepted skip. Invalid available evidence returns exit code 1. The explicit unavailable-evidence check passed. Command paths in historical raw receipts remain untouched; the verifier checks their relationship to the recorded artifact root rather than requiring a historical machine-specific directory.

Preserved setup history includes the initial ambiguous integer counter test compile failure, the test's intermediate arithmetic overflow, and a private saved-verifier root-depth substitution assertion. Those authored setup mistakes were corrected narrowly; they did not change production behavior, source/request goldens or successful raw outcomes. The original getter ceiling failure and stricter stream limit remain explicit qualification history. Full engine unit/focused/doc checks overlap and must not be summed as distinct runtime coverage.
