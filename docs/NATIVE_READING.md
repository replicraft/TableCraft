# Read-only native format inspection

`crates/engines/tablecraft-ace` is the first working independent native library.
Its current `inspect_header` API takes a caller-supplied Read + Seek source,
ResourceBudget and cancellation callback. It performs no writes, file discovery,
provider activation, password attempts, linked-source access or application startup.
It depends only on core values/budgets and thiserror; no TableCraft host is needed.

`tablecraft inspect <path>` opens that explicit file read-only and buffers a small
header summary before stdout. Errors exit 2 without successful partial output or
file paths in diagnostics. Existing output I/O/broken-pipe behavior is retained.
Use a stable/offline copy: native shared-file locking and consistent concurrent
snapshot acquisition are unimplemented. Host Read/Seek callbacks own I/O timeouts.

## Header profile

The reader recognizes Jet 3/4 and ACE header codes 2, 3, 5 and 6. It reports format
family, raw compatibility subversion, inferred page size, whole-file page count
and whether the obfuscated page-encoding key is nonzero. This identifies header
fields, not the creating Access version/build, successful decryption or a valid
application. Unknown codes/signature combinations are rejected explicitly.

Read size is 66 bytes, charged before reading; file length is bounded to 64 MiB,
page-aligned and at least three pages. Header recognition alone does not decode
catalogs or records. The zero-key encoding constant is format metadata, not a
password key or an implementation of cryptography. Header inspection can report
encoded pages; their content remains unsupported.

## Research provenance

Primary implementation references reviewed on 18 October 2026:

- [MDB Tools format research](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/HACKING.md): native page/signature/catalog structure research.
- [MDB Tools header reader](https://github.com/mdbtools/mdbtools/blob/6a8a4b0785c20f683d2460c9016f5f52039bd8ec/src/libmdb/file.c): header offsets, obfuscation and encoding-key interpretation.
- [Jackcess JetFormat](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java): independent format constants and family differences.

MDB Tools library sources carry LGPL-2.0-or-later notices; Jackcess is Apache-2.0.
These were inspected as technical references in a temporary research directory.
No upstream source implementation, fixture, library or license text was copied
into production code or linked into the executable. Rust inspection code and
synthetic test inputs are original. Reverse-engineered reference agreement is not
Microsoft Access comparison evidence or a complete published file specification.

## Observed fixture result

A disposable byte-identical copy of the [registered MIT example](FIXTURES.md)
reports Ace2007, subversion 0, 4,096-byte pages, 827 pages and an encoded-key field
representing zero. The original file remains unchanged. No macros, VBA, object
activation or data queries were executed. This result qualifies header inspection
of that candidate, not native catalog/application compatibility.
