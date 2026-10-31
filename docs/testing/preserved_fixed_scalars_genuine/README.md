# Fixed-scalar rich source recipe and read-only getter

These repository-authored Java tools prepare and inspect synthetic fixtures for
the [preserved fixed-scalar profile](../ACE16_PRESERVED_FIXED_SCALARS.md). They
use the pinned Jackcess 5.0.4 API and the existing
[BoundedFileChannel](../parent_index_split_originals/BoundedFileChannel.java).
They have not yet been compiled, executed or qualified. No generated database,
JAR or class file belongs in this directory.

The creation recipe derives from
[PreservedShortTextOriginals](../preserved_short_text_originals/PreservedShortTextOriginals.java).
The separate getter derives from the repository's
[PreservedInt32Originals](../preserved_int32_originals/PreservedInt32Originals.java)
inspection implementation. Those predecessor sources, classes and recorded
originals remain unchanged. No third-party implementation or fixture is copied.

## Arguments and effects

With the reviewed JAR and bounded-channel class supplied on the classpath, the
two entry points take exactly these arguments:

```text
PreservedFixedScalarOriginals ABSENT_OUTPUT_PATH V2016
PreservedFixedScalarOriginals ABSENT_OUTPUT_PATH V2019
PreservedFixedScalarGetter --inspect EXISTING_INPUT_PATH
```

The generator creates only the caller-selected absent output, using CREATE_NEW
and a 2 MiB bounded channel. It neither chooses a default output nor creates
parent directories. Unsupported profiles and existing paths refuse; a failure
after creation can leave a partial output for diagnosis. It does not repair,
overwrite or delete an existing database.

The getter opens its supplied channel with READ and opens Jackcess read-only.
It has no creation or update branch and writes only tagged inventory to stdout.
Both tools disable expression evaluation, select LocalDateTime carriers and
refuse linked target resolution. They never explicitly execute stored macros,
SQL, scripts or other active database content. External controllers own process
timeouts and stdout/stderr bounds; a channel bound is not a process memory limit.

## Frozen creation recipe

`PreservedRows` contains eleven columns in this order:

| Column | Public builder type | Recipe role |
| --- | --- | --- |
| ID | LONG | Manual primary key |
| Large_Number | BIG_INT | Ordinary nullable Int64 target |
| Date_Field | SHORT_DATE_TIME | Ordinary nullable legacy DATE target |
| Time_Field | SHORT_DATE_TIME | Second ordinary nullable legacy DATE target |
| Currency_Field | MONEY | Ordinary nullable scale-4 Currency target |
| Base | LONG | Indexed calculated input |
| Cached | Calculated LONG | Stored expression `[Base]*2` |
| Flag | BOOLEAN | Preserved sibling |
| Memo | MEMO | Preserved sibling |
| Blob | OLE | Preserved sibling |
| Files | Attachment | Preserved complex association |

The four targets are not indexed or calculation inputs and have Required false.
Their actual native numbers, offsets, flags, mask locations and record layouts
must be measured from the generated sources before any expected images or
producer calls. The eleven-field record requires two presence-mask bytes.
No Date/Time Extended fields are introduced.

Rows have IDs 1, 2 and 3. Their Int64 values are 1000, NULL and -2000; Currency
values are exact decimal strings 1.2345, NULL and -2.0001. Date values are
1899-12-30 midnight, NULL and 2024-02-29 midnight. Time values use the
1899-12-30 date with noon, NULL and 06:00. No floating-point construction of
Currency occurs. NULL inactive bytes are whatever the public writer produces;
the recipe does not patch them.

Base values are 7, 11 and 13. Expression evaluation stays off while explicit
stale Cached values 999, -333 and 0 are inserted. Boolean, Memo and Blob values,
the deterministic 8192-byte pattern and attachment populations 2/1/0 retain the
predecessor recipe. Attachments have explicit 2024-02-29 noon timestamps.
`Sentinel` retains its independent one-row marker and binary payload.
PublishToWeb BYTE0 is chosen when both tables are created. Generated system
timestamps/GUIDs are pinned afterward; repeatable logical inputs do not promise
byte-identical regeneration.

## Inventory and exact carriers

The getter reports complete selected table/column/property/index metadata,
owned-map references, source row ordinals and logical RowIds, all cell tokens,
the attachment flat table and attachment encoded/decoded data hashes. It bounds
tables to the recipe's three parent/flat rows and one Sentinel row, with at most
32 columns, 16 indexes and 128 properties per property map.

Existing tokens remain exact: NULL `N`, signed `I16`/`I32`/`I64`, unsigned `U8`,
Boolean `B0`/`B1`, UTF-8/base64 Text `T`, binary length/hash `X`, LocalDateTime
`LDT` and complex foreign key `FK`. The only additional carrier is BigDecimal:
it must have scale exactly 4 and its unscaled integer must fit signed i64. The
getter emits `CUR:` plus `toPlainString()` without rounding, rescaling, locale
formatting or conversion through floating point. Unknown carriers refuse.

LDT is a calendar observation, not a raw DATE-bit proof. Signed zero and temporal
aliases require independent raw framing and complete-image expectations even
where a getter would print the same timestamp. The getter's version-pinned
reflection reads only known index/map metadata; it does not invoke setters.

## Qualification boundary

Both tools and the unchanged bounded channel compiled successfully against the
cached Jackcess 5.0.4 JAR on 31 October 2026. The private
`.cache/ace16-preserved-fixed-scalars-java-compile-first-20261031/` receipt freezes
the exact compiler inputs and records exit 0. This was compilation only: it
created no database and invoked no getter.

Generation, original SDK inventories and direct complete native source admission
must pass before independent expectations or candidates are produced. The first
two creation and two read-only inventory calls now pass, followed by direct
complete native source admission of both originals. The
[source result](../results/ace16-preserved-fixed-scalar-genuine-sources-2026-10-31.json)
pins the unchanged originals, classes and logs. No update candidate was involved
in those checks; expected-image and producer qualification remain separate.
Native Microsoft Access save/reopen, required-macro
discovery and normal ACE saves remain separate qualification gaps.
