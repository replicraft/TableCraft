# Physical secondary-index creation sources

Status: SOURCE-qualified literal corpus; no creation writer or CLI is implemented.

The [source gate](../../crates/engines/tablecraft-ace/src/table/mutation/create_index_tests.rs)
constructs eight authored BEFORE images and sixteen complete ASC/DESC expected
images across headers 5 and 6. Its first execution passes one function covering
16 pairs and 32 complete reader/ownership views, before any new writer/API.
These are literal engineering fixtures, not Access-created databases or native
Access creation/save qualification.

Each source has Id primary/index aliases, nullable unindexed Score, Guard
secondary/index aliases, and a separate Sentinel table. The expected image adds
a genuinely new ordinary nonunique physical Score index, including NULL entries;
it does not reuse an existing access path. No new primary, foreign, UNIQUE,
Required or IgnoreNULL policy is qualified.

| Population | Rows | Fresh distinct-key counter |
| --- | ---: | ---: |
| Empty | 0 | 0 |
| Scores -1, 0, 1 | 3 | 3 |
| NULL, -7, -7, 0, NULL, 7 | 6 | 4 |
| Fixed mixed NULL/nonNULL pressure | 600 | 541 |

The pinned independent Jackcess writer initializes the new count descriptor to
zero and populates every original logical ROW. Ordinary flags 0x80 leave
IgnoreNULL unset. Duplicate key families retain distinct ROW entries without
incrementing that family counter; NULL contributes one family when present.
These initialization facts come from reviewed writer code, not the reader's
acceptance of arbitrary historical counters. Existing counters 1000/2000 remain
unchanged. This reference profile is not Microsoft Access conformance evidence.

The gate requires every old/new logical index to be Checked, compares complete
KEY/ROW streams, validates physical identities and map references, and runs the
actual source-wide DATA/INDEX/map ownership census. Old rows, aliases, histories,
Sentinel and every unrelated byte remain intact. It separately checks relocated
definition sections, exact GLOBAL bit changes and the inactive definition suffix.
The expected new container/root are pages 24/25, with pressure leaves 26/27;
the single-page definition grows from 500 to 622/624 bytes. These are explicit
literal model choices, not claims about native allocation order.

```sh
cargo test -p tablecraft-ace --all-features --lib --locked \
  qualify_real_secondary_index_source_and_complete_expected_before_writer
```

The [result](results/ace16-secondary-index-creation-sources-2026-10-30.json) pins
the model, source gate, helpers, review and first log. A bounded definition
composer, fresh-index allocation capability, creation API, independent producer
proof and actual CLI/shared-service consumer remain unimplemented. Unknown
tails/chains and wider policies must not be admitted by inference. Blank-database
creation, general CREATE/ALTER TABLE, required macro coverage, transactions and
normal ACE saves remain separate unfinished work.
