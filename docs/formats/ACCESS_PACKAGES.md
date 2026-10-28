# Inert Access package codecs

`tablecraft-formats`, at `crates/formats/tablecraft-formats`, owns the working
ZIP/OPC, XML and CAB readers extracted from the Access adapter. It reads inert
package contents and qualified stored template data. This extraction adds no
format support, database creation, template instantiation, encryption handling
or signature verification. ACE 16 remains the primary compatibility target;
SQLite and `.tcraft` implementation work remain paused.

## Ownership and direct imports

| Owner | Working responsibility |
| --- | --- |
| `tablecraft-formats::template` | `TemplatePackage`, part and relationship inventories, `TemplateObject`, inline schema discovery and complete `TemplateTable` values |
| `tablecraft-formats::deployment` | Bounded CAB framing/decompression, memory capture, host-storage extraction and opaque decoded payload facts |
| `tablecraft-formats::ReadError` | Payload-free I/O, cancellation, resource and allocation failures |
| `tablecraft-access` | Native database qualification and `AccessInput`; the query-facing `TemplateSource` adapter |
| CLI/other hosts | Input selection, budgets, cancellation, storage factories and cleanup |

Consumers import package APIs directly from `tablecraft_formats`; the Access
adapter does not re-export the moved package/error types. The CLI uses these
imports for template inspection and stored-data commands. It imports
`TemplateSource` from `tablecraft_access` when explicitly querying template data.

The formats crate depends on shared values, budgets, I/O, schema and object
metadata plus its actual codec libraries. It imports neither the native ACE
engine nor query execution, SQL syntax or the Access adapter. Physical database
interpretation remains in ACE. Query source capabilities remain in
`tablecraft-query`, with concrete routing and native I/O in the Access adapter.

## Template package and stored-data profile

`TemplatePackage::open` validates the bounded ZIP/OPC inventory and required
Access template metadata relationship. `open_if_template` selects by ZIP content:
non-ZIP inputs return `None` after rewind; invalid or unsupported ZIP packages
fail instead of silently falling through to a native reader. Opening exposes
immutable parts and relationships without extracting files or running bodies.

The current package profile supports classic single-disk ZIP with stored or
deflated entries, CRC/header agreement, checked part IRIs, content types and
internal relationships. Encrypted entries, ZIP64, split archives, unsupported
compression and external relationship targets remain refused. Package and total
expanded bytes are each bounded at 64 MiB, with at most 4,096 physical entries.
The detailed name, relationship and preservation rules remain in
[Access template inspection](../access/containers/ACCESS_TEMPLATES.md).

`objects` discovers inert object metadata. `table_columns` and
`columns_for_object` read the qualified local inline-XSD schema without decoding
row values. `read_table` returns a complete validated table; missing or linked
data is refused rather than invented or fetched. The implemented scalar profile
includes qualified text, Boolean, integral, floating, Decimal, date/time and
binary representations, with exact declared-domain checks and NULL handling.
The current limits are 256 columns and 100,000 rows, subject to the caller's
shared resource budget. See [stored template data](../access/containers/ACCESS_TEMPLATE_DATA.md)
and [List Definition agreement](../access/containers/ACCESS_TEMPLATE_LISTS.md)
for the admitted schemas, encodings and inert declaration rules.

`TemplateSource` retains metadata-only binding, complete scans before callbacks,
shared host budgets and schema checks in the Access adapter. Stored query and
application bodies do not become executable by appearing in a package.

## CAB payloads and native qualification

`deployment::read` captures the qualified single CAB member within the existing
64 MiB package and decoded-payload limits. `read_spooled` extracts to fresh
caller-owned `Read + Write + Seek` storage under its separate checked envelope.
`read_with_spool` retains small-image capture and lazily requests storage for
larger qualified inputs. Failed decoding is not retried with another strategy.
The Access adapter uses `read_validated` and `read_with_spool_validated` to
validate captured payloads after complete decoding and before the terminal
cancellation/work check. This callback applies only to captured bytes; spooled
payloads follow the explicit prefix-validation and rewind sequence below.
The existing CAB profile admits its checked uncompressed, MSZIP and LZX forms;
unsupported geometry or compression still refuses. See
[bounded ACCDC spooling](../access/containers/ACCDC_SPOOLED_READS.md) for source,
storage and size requirements.

A spooled result exposes `SpooledPayload::prefix`, `length`, `rewind` and
`into_inner`; `Selected` distinguishes captured bytes from spooled storage.
The decoder checks complete extraction and length, then captures an opaque
66-byte prefix. `into_inner` returns storage at its current position; it does
not imply native validation or rewind. The explicit `rewind` is paid and
cancellable. The Access adapter parses the native header from the prefix before
the paid rewind/check, then applies the native ACE-family gate before publishing
an `AccessInput`. Host spool-factory errors retain their original native error
class at that boundary.

Package signatures remain unverified. Decoded bytes grant no execution trust,
write authorization, native save/reopen qualification or global database-health
claim.

## Errors and verification boundaries

`TemplateError` and `DeploymentError` distinguish unsupported package profiles
and malformed structures from `ReadError`. The neutral reader error retains
portable I/O kinds, truncation, malformed reader results, cancellation, shared
limits, allocation failure, structural ceilings and lock contention. Errors omit
paths, part names, package contents and upstream diagnostic payloads. Conversion
from shared I/O errors stays in the formats layer; mapping to native or query
errors belongs to the corresponding adapter.

The owning crate contains package, encoding, relationship, scalar-data and
bounded-reader tests. Access retains adapter tests for source binding/scans and
native payload selection; CLI tests cover its direct consumers. These checks
establish the implemented boundaries, not native Access application parity or
support for unqualified template schemas and package variants.
