# Inert ACCDT package inspection

The Access backend now exposes `template::TemplatePackage::open` and immutable
part/relationship inventories. Basic CLI `inspect PATH` selects this reader by
ZIP content and the required template relationship, independently of extension.
It reports content parts, the metadata part, content types, byte lengths and
internal relationship counts. No file is extracted, instantiated or executed.

```sh
tablecraft inspect example.accdt
tablecraft inspect example.accdt --format tsv
```

This package layer now feeds [bounded stored table reading](ACCESS_TEMPLATE_DATA.md).
Catalog/column inspection, tables/counts, rows/pages and query run support generic
Object tables with qualified inline XSD. Qualified [List Definition agreement](ACCESS_TEMPLATE_LISTS.md) is now available;
broader defaults/types and other XSD/application layouts and native Access template qualification remain
unfinished. Metadata/object bodies remain inert without complete application-schema
validation. No file is instantiated and no external provider runs.

## Current package profile

Classic single-disk ZIP, stored or deflated entries, valid CRCs, consistent local/
central headers, bounded nonoverlapping entries and exactly one package-level
Access template metadata relationship are required. ZIP64, split archives,
encrypted ZIP entries, symlink entries and other compression methods
are refused. Empty directory records with validated names are ignored after header
and CRC checks; they still consume entry-count, metadata and work allowances.
They are not parts, relationship targets or candidates for content-type matching.
Nonempty directory records and exact duplicate physical names fail. Directory
case variants do not define competing logical parts.

Entry names map to relative, unambiguous logical part IRIs. Percent-encoded UTF-8
maps to Unicode; ASCII reserved escapes remain opaque (`%20` remains `%20`,
`%25` is not decoded twice). A literal plus differs from `%2B`. Raw UTF-8 ZIP names
remain accepted as a compatibility extension; the current OPC ZIP mapping calls
for ASCII names with non-ASCII characters escaped. Duplicate logical names are
rejected using ASCII case folding, including raw/escaped Unicode aliases. A part
cannot also be the parent prefix of another part. Empty segments, trailing-dot
segments, malformed UTF-8/escapes, escaped ASCII unreserved characters, encoded
slash/backslash, literal spaces, controls and raw query/fragment delimiters fail.
Colons are allowed inside package paths. No filesystem normalization or extraction
is used. Unicode NFC/NFD aliases are not normalized or merged.

The OPC content-types part resolves defaults and ASCII-case-insensitive overrides for each
content/control part. Namespace-qualified relationship parts require unique
identifiers and existing internal targets. Relative parent segments resolve
inside the package; absolute package paths work. External targets and root escapes
fail. Unknown internal relationship types remain visible and inert; their object
semantics are not inferred. Part lookup, overrides, relationship control paths and resolved targets use the
same ASCII-case-insensitive identity. Original content-part spelling is retained
for display and relationship target getters. Relationship type URIs remain exact;
Unicode letters are not case folded. Equivalent overrides are rejected as
ambiguous, just like equivalent ZIP entries. The special ZIP content-types item
still requires its exact reserved spelling.

The `iri-string` library validates RFC 3987 grammar and resolves internal URI/IRI
references. Reference unreserved escapes normalize before root-traversal checks;
UTF-8 targets and package paths can select the same logical part. Scheme,
authority, query and fragment references remain unsupported, as do references
above the package root (an explicit stricter policy than RFC root clamping).
Public `part()` lookup takes the logical names exposed by the inventory/targets;
it does not interpret a raw ZIP name. Interleaved pieces and wider package
features remain unfinished.

Microsoft documents the mandatory [OPC ASCII part-name equivalence and override rules](https://learn.microsoft.com/en-us/dotnet/core/compatibility/core-libraries/8.0/system-io-packaging-case-insensitive-uri).
This identity does not change SQL field/table name comparison.

Limits are 64 MiB compressed input, 64 MiB aggregate declared expanded bytes,
4,096 entries and 1,024 UTF-8 bytes per name, further restricted by the host's shared
ResourceBudget. Geometry/count/expanded-size checks precede dependency metadata
and decompression. Entry copying checks cancellation per 32 KiB chunk. Input,
expanded content, estimated metadata (8 KiB per entry), central-directory copies and decoder work
are charged cumulatively. The CLI retains its existing 16 MiB shared read allowance;
the format ceilings do not override that budget.

OPC control XML is UTF-8, at most 8 MiB per part, with at most 4,096 records,
32 attributes per element and root/record nesting only. XML bytes, records,
attribute copies and work are charged. DTDs, processing instructions, unsupported
attributes/elements, malformed or repeated roots and external entities fail.
These limits apply to control parsing; content bytes are not treated as control
XML merely because their content type contains XML. Failure publishes no package
or partial CLI output. Tracing records only a payload-free operation span.

Signatures are **not verified**. A well-formed package/relationship inventory
confers no signer trust, writable capability or permission to execute code.
Opening a template does not create a database or access a linked data provider.

## Evidence and next work

Six focused tests cover stored/deflated content, overrides and prefixed namespaces,
internal target resolution, corruption/CRC/header mismatches, ambiguity, external
and missing targets, XML refusals, quotas and every observed cancellation point.
Generated byte mutations additionally check bounded failure without panics.
The original [Python ZIP/OPC runner](testing/compare_template_packages.py) passes
62 CLI controls using independently written packages and unchanged-byte checks.
Fixtures contain original inert XML and are generated in temporary directories;
no third-party implementation or binary is copied.

These checks qualify this package profile only. Access-created template evidence,
complete object/list schemas, schema agreement and wider package qualification
remain unfinished. Bounded generic-Object inline data now supplies typed rows,
browsing and query integration under its separate contract. Full ACCDT reading stays on the backend roadmap.

Microsoft specifies [template packaging](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/e788b694-5581-4d9f-9730-af9ae59f9de8),
the [part/relationship map](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/9ed86b27-3af8-4f38-833c-5a72bcfb0d0d)
and the [unique metadata relationship](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/744f0320-ae8a-4344-bec2-14d950b6fb9e).
[List Data](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/606146d3-514f-4555-9d7e-039be4b3f40c)
describes the inline schema/data boundary still to implement. These references
supply format requirements, not native Access execution evidence.


The URI profile follows [ECMA-376 Part 2, fifth edition](https://ecma-international.org/publications-and-standards/standards/ecma-376/),
sections 6.2.2 (part syntax/identity), 6.4 (references), 6.5 (relationship paths)
and 7.3.3–7.3.5 (ZIP mapping). The implementation uses bounded strings only;
using a URI library does not create a network provider or execute part contents.
