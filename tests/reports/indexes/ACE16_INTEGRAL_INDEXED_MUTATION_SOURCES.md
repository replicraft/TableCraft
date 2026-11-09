# Int64 and Currency mutation source qualification

This source evidence preceded matching normal index mutations. Subsequent
producer results are recorded separately in the
[execution record](../../results/indexes/ace16-normal-integral-indexed-writes-2026-11-01.json).

Two unchanged SDK-created originals contain Int64 and Currency tables with
manual Int32 identities and fixed siblings. Literal requests cover MIN, NULL,
MAX, duplicate key families, a new zero family, batches and row deletion. Each
table has three physical root leaves exposed through six logical aliases,
including ascending and descending keys. Original inactive bytes under NULL
masks remain opaque and preserved.

Thirty-two independently constructed AFTER images plus the two originals passed
284 exact read-only SDK observations and saved verification. The native source
gate then checked all 34 views using full rows, aliases, constraints, counters,
ordinary Source protection and independent raw whole-image overlays. No matching
numeric mutation producer supplied an expected byte or ran during qualification.

The initial original preflight assumed zero inactive bytes under NULL; the SDK
preserved earlier MIN payload bytes instead. The originals and failed receipt
remain unchanged. A separate read-only validator corrected that assumption.
The first native command selected an ignored test without enabling it and ran
zero tests; the second explicit invocation passed the actual gate.

Exact manifest, source gate and reader receipt pins are in the
[source record](../../results/indexes/ace16-integral-indexed-mutation-sources-2026-11-01.json).
The gate lives beside the other native source tests in
[integral_indexed.rs](../../../crates/engines/tablecraft-ace/src/table/mutation/normal_ordinary_sources/integral_indexed.rs).
Native Access save/reopen remains unrun; source qualification alone establishes
no mutation permission.
