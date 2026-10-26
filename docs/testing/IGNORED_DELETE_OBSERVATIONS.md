# Original ignore-NULL DELETE observations

The [candidate receipt](results/ace16-delete-ignored-original-replay-2026-10-26.json)
records **50 positive candidates, zero disagreements**, on frozen engineering
example `b193984510d924a1688b8de2c5d073de90142af2`. Every selector succeeds without
waivers or budget changes. The candidates remove 3,346 roots, check 8,584 surviving
target rows and retire 136 original index pages. This is independent public-Jackcess
comparison, not native Microsoft Access execution or save/reopen qualification.
Normal writes, UPDATE eligibility, data macros and journal/apply remain separate.

## Reused originals and reference provenance

No database was regenerated and no index flags were edited. Six registered ACE16
V2016/V2019 originals were reused, with their existing hashes checked against the
[fixture ledger](FIXTURES.md) and owning observations:

- [DeleteStorage](DELETE_STORAGE_OBSERVATIONS.md): six Indexed rows, ordinary and
  ignored GroupKey indexes, physical aliases, descending/composite/unique indexes,
  Memo/OLE payloads, plus separate Unindexed and Sentinel tables.
- [IgnoreNullIndex](IGNORED_NULL_INDEXES.md): 701 rows, 637 ordinary ignored-key
  entries and 647 unique ignored-key entries, with ascending/descending trees and
  a shared physical alias.
- [IgnoredIndexCoverage](IGNORED_INDEX_COVERAGE.md): the existing UniquePartial,
  TupleGuid, TupleDecimal, ScalarTEXT0, ScalarTEXT1 and ScalarBINARY tables. No
  eligibility is inferred for the other tables in those files.

[IgnoredDeleteReference.java](IgnoredDeleteReference.java) is original repository
code under the root license. It only copies/deletes or performs inert getters;
it has no database-generation mode. Fresh reference copies use public
`Table.deleteRow`. Expressions are disabled, time zone is UTC, linked opens throw,
and getter reopens are read-only. The reviewed Jackcess 5.0.4 JAR has SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
No third-party implementation or fixture is copied.

Public row/index cursors establish values, identities and encounter order.
Identified implementation getters expose roots, encoded entries, ownership and
historical counters against that exact JAR; they are not a stable native API.
Independent raw checks match complete encoded key/reference sequences. Before
copying, all sources have plain ACE16 headers, subversion zero, unencoded pages,
normal catalog flags, matching stored/current column counts, no selected-table
properties, relationships or children, and NULL active payload fields. These facts
do not establish general macro absence; the frozen candidate separately runs its
existing full before/post and constraint gates.

## Fifty unchanged selectors

Each profile has the following 25 reference/candidate selections:

| Target | Selections per profile |
| --- | --- |
| Indexed | NULL GroupKey IDs 3/4; eligible ID 2; mixed IDs 2/3; all four GroupKey-eligible IDs |
| Ignored | both keys NULL ID 0; only KeyValue NULL ID 11; only UniqueValue NULL ID 13; mixed IDs 0/1/11/13; all 637 KeyValue-eligible roots |
| UniquePartial | all-NULL IDs 0/1; partial-NULL IDs 2/3; mixed IDs 0/2/4; all four eligible roots |
| TupleGuid, TupleDecimal | all 16 all-NULL roots; first two actual partial-NULL roots; all 185 eligible roots, for each table |
| ScalarTEXT0, ScalarTEXT1, ScalarBINARY | all 19 NULL roots and all 182 eligible roots, for each table |

Membership comes from actual indexed-column metadata and getter NULL facts,
separately for every index. Only an all-components-NULL key is omitted; partial
NULLs remain indexed. The key and unique indexes can have different effective
subsets for the same requested roots. Physical aliases are deduplicated by actual
physical identity, never by payload equality or assumed names.

The model checks **32 effective-empty per-physical-index controls** with exactly
unchanged index pages, and **24 empty physical-index results with live NULL table
rows**. These counts deduplicate aliases within each control. Historical counters
are preserved rather than treated as live populations. Memo, variable Text/Binary
composites, unique overflow and all existing unsupported profiles remain outside
this DELETE qualification; index checking alone does not grant write authority.

## Sequential references versus atomic candidates

The [reference receipt](results/ace16-delete-ignored-original-reference-2026-10-26.json)
preserves 50 fresh copy hashes, their getter/request receipts, 106 successful
copy/getter commands and independently audited allocation transitions. All six
originals and all 50 references remain unchanged during candidate replay.

Public sequential deletion rewrites pages before their eventual retirement.
All 136 retired original index pages therefore have reference residual bodies
that differ from the original, despite zeroed four-byte retirement prefixes.
Those differences and both residual digests remain in the pinned raw audit.
Candidates enforce a stronger preservation policy: their retired residual bodies
remain exactly the original bytes.

The two ScalarTEXT1/all-eligible references also grow by six pages. Their Ordinary
indexes retain 19 NULL roots but grow from height 1 to 2, adding one node. Both
Ordinary and Ignored owned maps become indirect, allocating two bitmap pages;
the Ignored indexes finish empty at their original roots. Global allocation
changes, map representations and future bits are recorded explicitly.

The same two candidates succeed without reproducing those sequential intermediate
splits. They preserve original file length, nonempty height 1 and inline owned
maps, with identical final values, full retained key/reference bytes and eligible
populations. The checker independently verifies original leaf partitions, full
key-and-row separator maxima, original node forms, exact aggregated map bits,
slot tombstones, counts, residual bodies and every other byte. It does not require
candidate topology or file length to equal the sequential reference.

## Source versions and reproduction

Historical preparation and audit used private sources preserved as inert text:
[prepare](ignored_delete_originals/historical/prepare.py.txt),
[audit](ignored_delete_originals/historical/audit.py.txt), and the excluded
[JSON-ID](ignored_delete_originals/historical/audit-before-json-id-fix.py.txt),
[residual](ignored_delete_originals/historical/audit-before-reference-residual-observation.py.txt)
and [growth](ignored_delete_originals/historical/audit-before-reference-growth-observation.py.txt)
versions. The first failure was an authored string/integer ID mismatch. The other
two failures exposed unjustified assumptions about sequential reference bodies
and file length. Correcting the reference audit did not weaken candidate policy.
Exact raw logs/source hashes remain in the receipts.

The public [reference](ignored_delete_originals/reference.py) and
[audit](ignored_delete_originals/audit.py) tools accept explicit paths, are import
safe and write fresh outputs only. The public
[replay](ignored_delete_originals/replay.py),
[atomic checker](ignored_delete_originals/atomic.py) and
[support](ignored_delete_originals/support.py) are byte-identical promotions of
the actual candidate tools. The public
[getter parser](ignored_delete_originals/getters.py) differs only by removal of
two trailing blank lines at EOF; its Python AST is unchanged. Receipts retain the
immutable private execution hash separately from the current public source hash. Shared raw framing is reused
[read-only](encoded_delete_originals/wire.py). Repository-relative artifact labels
and outside-repository rejection prevent machine-specific public labels.

Separate public verification re-read all retained references and recomputed their
allocation audit. A fresh public-Java compile and one copy/getter smoke verify
helper reproduction; that smoke is not another accepted selector. No candidate
was rerun merely for publication. Expanded getter/key arrays, raw reports and
databases stay ignored under `.cache/ignored-delete-originals-next-20261026/`.
Do not use Python `-O`, which disables the independent assertions.

Reproduction requires the retained, hash-matched originals and reviewed JAR;
fixture binaries are not shipped. Compile the public Java helper into a fresh
repository-local class directory. Supply the six inputs explicitly, for example:

```sh
javac -cp "$jar" -d "$classes" docs/testing/IgnoredDeleteReference.java
python3 docs/testing/ignored_delete_originals/reference.py \
  --input DeleteStorage V2016 "$delete2016" \
  --input DeleteStorage V2019 "$delete2019" \
  --input Ignore701 V2016 "$ignore2016" --input Ignore701 V2019 "$ignore2019" \
  --input Coverage V2016 "$coverage2016" --input Coverage V2019 "$coverage2019" \
  --helper-source docs/testing/IgnoredDeleteReference.java --helper-dir "$classes" \
  --jar "$jar" --workdir "$fresh_references" --report "$reference_report"
python3 docs/testing/ignored_delete_originals/audit.py \
  --references "$reference_report" --report "$transition_report"
python3 docs/testing/ignored_delete_originals/replay.py \
  --reference-report "$reference_report" --transition-audit "$transition_report" \
  --helper-source docs/testing/IgnoredDeleteReference.java --helper-dir "$classes" \
  --jar "$jar" --example "$frozen_example" \
  --example-sha256 eb21095d79f87283b23c0052102e3628b2adf1578d3cc6846ade6f7568e4891d \
  --compiled-revision b193984510d924a1688b8de2c5d073de90142af2 \
  --workdir "$fresh_candidates" --report "$candidate_report"
```

Run from the repository root with all artifacts inside the checkout. Every output
must be fresh. To verify existing reference copies without making new ones, use
`reference.py --verify-existing REPORT` with the same helper/JAR arguments and a
fresh `--report`; omit `--input` and `--workdir`. Accepted originals and historical
receipts must never be regenerated, overwritten or relabeled as public-path runs.
A later binary requires its own frozen hash, revision and build receipt.
