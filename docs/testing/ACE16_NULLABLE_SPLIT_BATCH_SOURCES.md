# Nullable batch index-growth sources

The source-only engine test
`qualify_nullable_split_batch_sources_and_complete_expected_images_before_preview`
checks 80 independently constructed source/expected image pairs before invoking
any mutation API. It passes across both ACE 16 selectors, both key directions,
UNIQUE/nonunique indexes and inline/forwarded logical rows.

Each source has 500 fixed records, an ordinary nullable Int32 index and an
ignored-NULL mixed-direction Int32 tuple index. Each physical index has two
genuine logical aliases. Five shapes cover two-owner root growth, an initially
empty ignored leaf, non-root growth, simultaneous clearing/new eligibility, and
fitted existing-branch retention. Expected images allocate four, one or zero
INDEX pages as appropriate. DATA pages, slots, forwarding pointers and ordinary
ownership/hint maps retain their source representation.

The source gate reads every source and complete expected image through the real
table, index and constraint readers. It checks all rows, four aliases, historical
counters, original headers and the shared allocation order. The expected images
were frozen before this read gate ran; the first execution passed all 80 cases.

This is source/model qualification, not a passing batch mutation implementation,
an original Access fixture, or native Access comparison. Allocation-capable
multi-owner batch UPDATE implementation and independent original-request audit
remain the next work. Multi-table allocation, general saves and required
data-macro semantics remain unfinished.

```sh
cargo test -p tablecraft-ace --all-features --lib --locked \
  --config profile.test.package.tablecraft-ace.debug=1 \
  qualify_nullable_split_batch_sources_and_complete_expected_images_before_preview
```

The local run used reduced test debug information after unrelated large test
builds exhausted disk. Assertions, optimization and incremental compilation were
unchanged. This does not claim a default-debug build of this gate.
