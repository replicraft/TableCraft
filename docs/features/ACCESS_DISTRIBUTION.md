# Access formats, distribution and signatures

Status: future qualification requirements, reviewed 18 October 2026. No native
format reader/writer, signature verifier, compiled-project author or package
extractor is implemented. Extensions identify candidates, not proven file content.

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
[database versus package signatures](https://support.microsoft.com/en-us/access/show-trust-by-adding-a-digital-signature-to-an-access-database)
and [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
are primary research inputs. Signing support and limits vary by Access version;
do not apply one profile's behavior to every historical artifact.
