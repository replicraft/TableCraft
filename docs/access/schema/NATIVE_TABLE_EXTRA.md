# Inert table Extra inspection

`NativeImage::inspect_table_extra` reads the selected local table's catalog
`LvExtra` payload. It preserves the exact assembled bytes and decodes the
observed wide-MR2 property framing under the existing native resource budget.
This is a storage reader, not a data-macro runtime or an absence detector.

The CLI exposes an explicit metadata report:

```sh
tablecraft inspect example.accdb --table Orders --extra
```

The report shows physical provenance, presence, framing, numeric group kinds,
property names, flags, types, byte counts and whether text validated. Stored text
and XML bodies are not printed. This option requires a table and conflicts with
other inspection modes. Errors remain buffered before stdout. It does not change
default inspection or table browsing.

The caller supplies a captured, catalog-scoped `ObjectId`. The result retains
that identity, the original native Id and ParentId, and the logical catalog
record page and slot. It does not select an object by a coincidentally equal name
or infer event ownership from a property name. Foreign identities and non-local
table objects refuse.

## Presence and framing

Physical presence remains separate from decoding coverage:

| State | Result |
| --- | --- |
| Catalog column missing | Uninspected presence, no bytes, NotPresent coverage |
| Catalog value NULL | Absent presence, no bytes, NotPresent coverage |
| Present zero-length payload | Present presence, empty bytes, Empty coverage |
| Supported complete framing | Exact bytes, WideMr2 coverage |
| Unknown signature or unsupported group metadata | Exact bytes, Opaque coverage |
| Malformed recognized framing | Typed error; no partial result |

The observed framing has an MR2 signature, a UTF-16LE name list, numeric group
kinds 3 and 4, a ten-byte empty-name metadata header and entries with 32-bit
lengths. Each property retains its name, numeric type and flags, and exact value
bytes. Flags 1 with type 12 additionally expose strictly validated UTF-16LE text.
Other types and flags remain opaque. Unknown groups retain their complete bodies;
unsupported name metadata keeps the group opaque. Neither group numbers nor
property names become event enums.

The existing narrow `LvProp` decoder is unchanged. These are distinct physical
profiles despite their shared signature. Table Extra inspection does not parse
XML, resolve named macro calls, execute actions or evaluate strings as SQL.

## Bounds and preservation

The reader projects only `LvExtra` through the existing catalog payload reader.
Unrelated `Lv` and `LvModule` payloads are not assembled or followed. The existing
all-payload API continues to require every requested payload to succeed.
Inline, external and chained storage retain their existing strict limits and
page/record, chain and length validation. This read operation does not establish
exclusive allocation ownership for mutation. Complete-image retention is unchanged.

Decoding retains the 2 MiB native payload ceiling and separately bounds name,
group and entry counts at 256, 1,024 and 4,096. Allocation and comparison work are
charged; loops and final publication check cancellation. Invalid lengths,
references, duplicate supported groups/properties and invalid supported UTF-16
return errors. No partial inspection escapes after failure.

## Compatibility boundary

Observed desktop-origin data-macro payloads motivated this profile. A property
name need not equal the event named by its XML text. An Access Services namespace
in stored text does not, by itself, classify the database as a web application.
Names-only payloads, missing/NULL bytes and successfully decoded content do not
establish qualified macro absence.

Required event semantics, absence qualification, native Access save/reopen and
normal ACE writes remain unfinished. Follow the
[data-macro qualification contract](../../testing/DATA_MACROS.md) before changing
write eligibility. [Opaque payload preservation](../writes/NATIVE_PRESERVATION.md)
continues to apply to every unimplemented object body.

For engineering comparison, the `table_extra` engine example emits an explicitly
requested complete hex representation of the selected payload, groups and
properties. It uses clap Parser, exact table selection and bounded buffered
output. Its output can contain private application content; normal CLI metadata
inspection uses the summary above.

```sh
cargo run -p tablecraft-ace --example table_extra -- example.accdb Orders
```

## Verification

Nine focused engine functions, seven existing payload/property regressions and
four CLI integration functions pass. The full engine library passes 675 tests
with two existing generators ignored. Strict all-target/all-feature ACE/CLI and
minimal ACE Clippy, formatting, documentation links and dependency checks pass.
Local library test builds explicitly use test debug level 1 after a disk-space
failure; assertions, optimization and incremental compilation are unchanged.

Seven original-file calls match 14,266 complete payload bytes, nine values and
54 output records against independent source-derived expectations. The four
original databases retain their hashes. These originals use ACE 2010 storage;
they do not qualify ACE 16 macro behavior. The generic
[comparison runner](../../testing/compare_table_extra.py) consumes an explicitly
supplied private manifest; no third-party database or macro body is tracked.
See the [record](../../testing/results/table-extra-inspection-2026-10-29.json)
for the captured evidence and earlier failures.
