# Desktop archive preparation

Desktop packaging is separate from the existing [CLI release](RELEASING.md).
The portable archive tooling does not install the application, register file
associations or publish a release. The [desktop checklist](../releases/DESKTOP_0_1_0_CHECKLIST.md)
retains the remaining release gates.

## Application icons

The headless `tablecraft-ui` example renders the original application SVG
directly at each resolution, preserving its transparent canvas. It emits nine
PNGs from 16 through 1024 pixels, a seven-image ICO and an ICNS with native and
retina image records. It uses existing SVG/PNG dependencies and opens no window.
The output directory must be new and its parent must already exist:

```sh
TABLECRAFT_ICON_DIR=.cache/desktop-icons cargo run --offline --locked \
  -p tablecraft-ui --example export_application_icons
```

The source artwork remains
[`application-light.svg`](../../assets/ui/icons/tablecraft/application-light.svg).
Generated files are build outputs, not replacement source artwork. Producing an
ICO does not embed it in a Windows executable; that resource step remains open.

## Portable archives

[`package_desktop.py`](../../scripts/packaging/package_desktop.py) accepts an
already built native desktop binary and an explicit, reviewed, complete desktop
notice bundle. The current `packaging/ThirdPartyLicenses.txt` covers the CLI;
it is not a reviewed desktop bundle. The tool checks the supplied notice bytes,
but cannot establish their legal completeness.

Archive names start with `tablecraft-desktop-<version>-`. Windows uses ZIP with
the executable, project license, notices and `RELEASE.json`. Linux uses tar.gz
with the same resources. macOS uses ZIP with a `TableCraft.app` bundle, including
the executable, generated Info.plist, supplied ICNS, license and notices. The
manifest hashes every payload file; a SHA-256 sidecar accompanies each archive.

After building the appropriate binary and reviewing desktop notices, a macOS
preparation command has this form:

```sh
python3 scripts/packaging/package_desktop.py package --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/tablecraft-desktop \
  --notices /path/to/reviewed/desktop/ThirdPartyLicenses.txt \
  --icon .cache/desktop-icons/application.icns --output .cache/desktop-dist
python3 scripts/packaging/package_desktop.py verify --version 0.1.0 \
  --target aarch64-apple-darwin \
  --archive .cache/desktop-dist/tablecraft-desktop-0.1.0-macos-arm64.zip \
  --notices /path/to/reviewed/desktop/ThirdPartyLicenses.txt \
  --icon .cache/desktop-icons/application.icns --smoke
```

Windows/Linux omit `--icon` in this initial portable contract. Verification
requires exact payload membership, bounded regular files and permissions,
matching architecture/version, canonical bundle metadata, reviewed legal/icon
bytes and manifest hashes. It rejects duplicate members, links and unlisted
paths before extraction. Output paths must be fresh. ICNS verification checks
the image records, optional table of contents and bounded PNG pixel streams.

`--smoke` runs only the extracted binary's `--help` and `--version`; use it on
the matching native host. It does not initialize the desktop or qualify display,
file dialogs, accessibility, file-manager opens or database behavior. Native
window checks remain suspended at the user's request.

## Remaining integration

No desktop CI matrix, publication integration, installers, signing,
notarization or working file associations are supplied by these utilities. The
archive target names reuse the CLI's six architecture identifiers without
claiming those desktop builds have been qualified. macOS deployment minimums and
Linux runtime-library requirements still need build evidence. Bundle metadata
deliberately declares no document handler; see the
[file-association boundary](../architecture/FILE_ASSOCIATIONS.md).

## Complete desktop asset set

[`desktop_release.py`](../../scripts/packaging/desktop_release.py) checks the
six archives and six sidecars together before creating assembly outputs:

```sh
python3 scripts/packaging/desktop_release.py finalize --version 0.1.0 \
  --output .cache/desktop-dist \
  --notices /path/to/reviewed/desktop/ThirdPartyLicenses.txt \
  --icon .cache/desktop-icons/application.icns
python3 scripts/packaging/desktop_release.py verify --version 0.1.0 \
  --output .cache/desktop-dist \
  --notices /path/to/reviewed/desktop/ThirdPartyLicenses.txt \
  --icon .cache/desktop-icons/application.icns
```

Finalization requires exactly twelve input files. It revalidates every archive
against the reviewed notice/icon inputs and checks every sidecar, then creates
`DesktopThirdPartyLicenses.txt` and `SHA256SUMS-desktop.txt`. Verification requires
exactly those fourteen assets and compares all thirteen payload checksums. The
standalone names differ from the CLI bundle so a future combined release can
retain both notice scopes. An absent platform, changed notice, invalid checksum
or extra file refuses assembly. Failed output writes remove only files created
by that invocation; existing archives remain intact. These commands do not
publish artifacts or establish native platform compatibility.

## Local verification — 7 November 2026

The thirteen desktop packaging/assembly tests and thirteen existing CLI
packaging tests pass.
They exercise synthetic native headers; this is not six-platform build evidence.
Four icon tests decode each PNG payload and check native container offsets and
dimensions against independent expectations. The exporter builds and runs;
Apple's `iconutil` independently extracts the generated ICNS. The 16/256-pixel
outputs and extracted 1024-pixel image were opened for offscreen review. No
artwork, font files or registry dependencies changed.

A local macOS ARM64 archive containing the existing debug desktop binary and
generated ICNS passed extraction and headless `--help`/`--version` smoke checks.
It used an explicitly synthetic test notice, remains in ignored cache and is
not distributable. This checks the archive path, not a release build or a
completed desktop notice audit.
