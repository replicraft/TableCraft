# Access formats, distribution and signatures

Status: read scope expanded 20 October 2026. The working native reader accepts
supported database headers independently of the filename extension. CLI regressions
exercise discovery/counts, rows, SELECT and inspection under `.accdr`, `.accde`
and `.mde` names without modifying their bytes. These use renamed ordinary
fixtures, not Access-produced compiled projects. No signature verifier,
compiled-project author or ACCDT reader is implemented. A bounded CAB deployment
reader now feeds native reads; its qualification is described below. Extensions identify
candidates, not proven file content.

## Required read scope

Reading local tables and stored template data in **all five of `.accdr`, `.accde`,
`.mde`, `.accdt` and `.accdc`** is an accepted requirement in the current read/query
work, independently of implementing their application behavior.

| Extension | Representation and intended read path | Current evidence / remaining work |
| --- | --- | --- |
| `.accdr` | Runtime-mode Access database; use header-selected ACE reader | Microsoft documents ACCDB renaming; CLI routing regression |
| `.accde` | ACE database with compiled VBA and editable VBA source removed; read local tables through ACE | Header routing regression; native compiled fixtures remain unqualified |
| `.mde` | Compiled Jet-era database; select its actual Jet generation | Jet 4 routing regression; native compiled fixtures and older layouts remain unqualified |
| `.accdt` | OPC ZIP template with related schema, data and application-object parts | Package/relationship validation, schema mapping and stored-data reading unimplemented |
| `.accdc` | Signed deployment package containing one `.accdb` or `.accde` application | Bounded single-database CAB reading works; signatures remain unverified; native package qualification pending |

Compiled/runtime suffixes do not need a separate scalar decoder. Header/version,
encoding, column and index checks still determine eligibility. Compilation does
not make user records compiled or unavailable. A distributed frontend may contain
links rather than local records; read commands must not retrieve those links
automatically or execute startup macros, event handlers or VBA.

ACCDT is not generally an ACCDB renamed to a template. Microsoft's
[package specification](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/eaca4861-cd0e-4130-8af5-ea92fd9a459a)
requires OPC ZIP packaging and a relationship to the template metadata part.
Read stored definitions/data directly, without instantiating an application.
Missing sample data does not authorize execution or external retrieval.

Package reading belongs above the page engine, initially in the Access backend's
feature modules. Hosts compose readers; CLI commands own arguments/presentation,
not archive or template semantics. Qualified template schema/values feed shared
read/query contracts; a contained native database reuses ACE and its encryption
and read limits. Add no empty format/container crates or duplicated native decoder.

Acceptance requires bounded compressed input, decompressed bytes, entries/parts,
XML nesting and row/value work with cancellation. Reject ambiguous/duplicate
parts, unsafe paths and unsupported relationships. Never follow external
relationships, install files or activate content. Report unverified signatures
as unverified; an edited database does not retain its package signature.

Qualify actual Access-created templates, signed packages and compiled projects
with provenance, hashes, Access build/bitness and expected local schema/rows.
Compare discovery, counts, rows and SELECT, unsupported fields/links, encryption,
corruption, resource exhaustion and unchanged originals. Renamed fixtures prove
filename routing only; no native compiled or package fixture is qualified yet.

## Working ACCDC read profile

`tablecraft-access::AccessInput` detects CAB by `MSCF` content and captures the
contained ACE database in memory. Ordinary native inputs remain streaming.
The CLI uses this input for `inspect`, `tables`, `rows`, saved-query retrieval,
`query run --database` and `encryption inspect`. Supported encrypted contained
databases use the ordinary `-p` / hidden prompt and native codec. The original
package stays untouched; no database is extracted to disk. Encryption rewrites
and table-update commands do not accept packages or silently produce a package
with an invalid signature.

```sh
tablecraft inspect application.accdc
tablecraft tables application.accdc --count
tablecraft rows application.accdc Contacts
tablecraft query run 'SELECT * FROM Contacts' --database application.accdc
```

Inspection labels `Container` as `Access deployment (CAB)` and `Package signature`
as `Not verified`, including unsigned CAB derivatives. No signer/trust result is
inferred from the extension, header reservations or trailing certificate bytes.
This does not reproduce Access's package trust or installation workflow.

Current profile: CAB 1.3, one folder and one file at decompressed offset zero,
no split/previous/next cabinets, a plain basename of at most 255 bytes, and a
contained recognized ACE database. Compressed input including any signature
overlay and decompressed database are each capped at 64 MiB, subject to shared
host limits. Header/folder/data reservations are bounded and retained only as
unverified container data. Compressed block bounds, aggregate expanded length
and geometry are checked before library decompression. Each output chunk is
bounded to 32 KiB with cumulative cancellation/steps; four MiB of decoder work
storage is charged. Quantum and unsupported compression parameters fail.
MSZIP and uncompressed fixtures pass exact-byte checks; the decoder also accepts
LZX windows 15–21, which still need Access-package fixture qualification.

Original independently laid-out CAB fixtures wrap the registered MIT example,
including password-encrypted ACE derivatives. Tests compare exact extracted
bytes, counts, rows and SELECT, reject missing/wrong credentials, truncation,
unsafe names, multiple/split entries, excessive sizes and cancellation/budget
failure. Reservation/overlay handling and generated corruption cases are covered.
These are container integration tests, not signed Access-produced fixtures or
signature verification. Native signed-package/Windows comparisons remain pending.

## Separate capability levels

| Candidate family | Qualification boundary |
| --- | --- |
| Jet-era `.mdb` generations | Identify generation, encoding/security profile and object layout independently |
| `.accdb` | Version-specific field, object and application features |
| `.mde` / `.accde` | Compiled distribution; preservation/inspection distinct from execution or creating new compiled projects |
| `.accdr` | Runtime-mode distribution; extension/mode is not a sandbox or substitute for underlying format detection |
| `.accdt` and legacy templates | Package/instantiate semantics, object relationships and template metadata |
| `.accdc` | Deployment package, contained database and package signature distinct from database code signing |
| `.mda` / `.accda` and project references | Add-in discovery, dependency resolution, activation and version/platform restrictions |

For each exact profile, report identification, inspection, preservation, execution,
editing, native authoring and signing separately. Preserve original bytes and
unknown objects where possible; do not fabricate VBA source for compiled files.
Keep legacy security/password/encryption handling profile-specific.

Record signed scope, verification result, certificate/chain policy and unsupported
algorithms. Preserving signature bytes does not establish validity. Before edits,
determine whether signatures would be invalidated; require an explicit choice to
produce an unsigned/invalidated derivative or reject the affected operation.
Never label a modified artifact as still signed without verification. Re-signing
requires a separate implementation and authorized key access; private keys do not
belong in fixtures or repository history.

Bound package extraction, reject unsafe paths and do not execute extracted content.
Inspection, execution trust and signing trust are separate decisions. Native
engine codecs own database fidelity; lifecycle/runtime own packaging and
deployment; security/platform contracts supply verification and optional key use.

## Version profiles and acceptance

Record file generation, Access/ACE build, bitness, locale, references and supported
features. Microsoft currently specifies larger form/report dimensions beginning
with Microsoft 365 Access Version 2609, build 16.0.20430.20146. Therefore capability
checks must include build-qualified design limits, not just an `.accdb` label.
This is documented upstream behavior, not observed TableCraft compatibility.

Compare signed/unsigned originals and edited copies in native Access, including
signature invalidation, compiled-project preservation and package extraction.
Use explicit skips for absent signer/oracle environments; no re-signing or native
distribution tests have run. See the [application corpus](../testing/ACCESS_APPLICATIONS.md).

## References

[Deploy an Access application](https://support.microsoft.com/en-us/access/deploy-an-access-application),
[Microsoft's MDE description](https://news.microsoft.com/source/2004/05/24/qa-microsoft-office-access-2003-conversion-toolkit-will-significantly-streamline-database-conversion-and-ease-deployment-of-microsoft-office-2003-for-customers/),
[MS-ACCDT](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/0a4a68d7-7a85-4a27-ad74-730db57862d7),
[database versus package signatures](https://support.microsoft.com/en-us/access/show-trust-by-adding-a-digital-signature-to-an-access-database)
and [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
are primary research inputs. Signing support and limits vary by Access version;
do not apply one profile's behavior to every historical artifact.
