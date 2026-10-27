# Public-writer data-page row limit

Fresh repository-owned [producer](RowSlotBoundaryFixture.java) originals use the
reviewed public Jackcess 5.0.4 builder. Each has one ordinary `LONG` column,
`Slots.N`, containing exactly the integers 0 through 599, with no indexes or
AutoNumber. Generation requires a fresh path; expressions are disabled, linked
opens throw, and getters open read-only.

The [compact receipt](results/row-slot-boundary-2026-10-26.json) pins the producer,
compiled class, reviewed JAR, four originals and all command transcripts. Nine
commands passed: one compilation and four generation/getter pairs. Each command
had a 30-second timeout; no retries or original mutations occurred.

| Public format | Data pages | Raw row counts | Maximum getter slot |
| --- | --- | --- | ---: |
| V2016 | 91, 93, 94 | 255, 255, 90 | 254 |
| V2019 | 108, 110, 111 | 255, 255, 90 | 254 |
| V2000 | 26, 28, 29 | 255, 255, 90 | 254 |
| V2007 | 82, 84, 85 | 255, 255, 90 | 254 |

For all four profiles, the actual JAR format reports
`MAX_NUM_ROWS_ON_DATA_PAGE = 255`. Public row locations cover every slot from
zero through each page's count minus one. Literal raw page counts agree with
those inventories, and every data-page owner matches the table definition.
The page size is 4096 bytes, with the count at offset 12 and two-byte slot words
beginning at offset 14.

The initial hypothesis that an eight-bit packed slot reference justified 256 rows
was rejected. These originals contain no count of 256 and no slot 255. The existing
255-row reader ceiling was not changed. The observations also provide a public
writer boundary for a subsequent hidden-slot append; they do not qualify broader
forwarding, Microsoft Access execution, native saves or complete format support.

Full getter rows and original database bytes remain ignored under
`.cache/row-slot-boundary-20261026/`. Eight separate frozen CLI controls pass
with no disagreements: all four exact
600-row TSV sequences and all four `Count(*) = 600` results, with empty stderr
and unchanged originals. The CLI was compiled at `596d40ea` and its complete
artifact hash and build-receipt link are retained in the compact receipt. All
selected build source pins were independently checked against that revision.
These controls establish existing bounded read behavior, with no engine change
or broader compatibility claim.
