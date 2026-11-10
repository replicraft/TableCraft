# macOS DMG packaging

Builds a custom TableCraft drag-to-Applications DMG from the checked portable
macOS update ZIP. Both supported architectures use the same source-generated
layout and retain the exact application bytes, including the in-bundle release
manifest. The updater continues consuming the portable ZIP; a DMG is the
interactive installation download.

Install packaging tools into an isolated Python environment:

```sh
python3 -m venv .cache/macos-packaging-venv
.cache/macos-packaging-venv/bin/python -m pip install -r packaging/macos/requirements.txt
```

After generating the actual desktop notices/icon and building its portable ZIP:

```sh
.cache/macos-packaging-venv/bin/python scripts/packaging/package_macos.py package \
  --version 0.1.0 --target aarch64-apple-darwin \
  --archive dist/tablecraft-desktop-0.1.0-macos-arm64.zip \
  --notices resources/DesktopThirdPartyLicenses.txt \
  --icon resources/icons/application.icns --output dist
.cache/macos-packaging-venv/bin/python scripts/packaging/package_macos.py verify \
  --version 0.1.0 --target aarch64-apple-darwin \
  --archive dist/tablecraft-desktop-0.1.0-macos-arm64.zip \
  --notices resources/DesktopThirdPartyLicenses.txt \
  --icon resources/icons/application.icns \
  --dmg dist/tablecraft-desktop-0.1.0-macos-arm64.dmg
```

Intel uses `x86_64-apple-darwin` and `macos-x64` filenames. Both the DMG and its
SHA-256 sidecar require fresh output paths. No installation, registration, native
application launch, tag or publication occurs.

`dmg_settings.py` controls the 720×400-point icon-view window, application and
Applications locations, custom volume icon, background and hidden Finder chrome.
`background.py` renders original TableCraft artwork using the existing unmodified
[Selawik fonts and license](../../assets/fonts/OFL-Selawik.txt). Rendered PNGs stay
in build staging; no generated background is checked in. PaintCraft and EarthCraft
provided layout/tool references; none of their logos, backgrounds or code were
copied.

[dmgbuild](https://dmgbuild.readthedocs.io/en/latest/) writes Finder metadata
without Finder or AppleScript. The wrapper explicitly adds `-noautoopen` to its
private writable staging mount. Verification uses `hdiutil verify` and native
image metadata, then a separate `-readonly -nobrowse -noautoopen` mount in an owned
private directory. It checks the actual `.DS_Store` placement/view/window records,
exact volume contents, the `/Applications` link, artwork/icon bytes and every
application file's bytes and permissions against the checked update ZIP. Unknown
files/directories and application symlinks refuse. The owned mount is detached
on verification failures. No Finder window is needed or opened.

Checksums and later Minisign release signatures are separate from Apple Developer
ID signing/notarization. This tooling does not claim those OS signatures or change
application file associations.

## Local evidence — 9 November 2026

A real ARM64 DMG built from the retained debug desktop binary passes native
`hdiutil` verification, the read-only payload/layout checks and same-application
comparison. The first native verification found that the DS_Store library
requires a string filename rather than a `Path`; the corrected native rerun
passes, and failure cleanup detached its mount. The background was reviewed
offscreen at 720×400 pixels on macOS ARM64. This is custom installer artwork;
there is no Microsoft Access installer capture being used as a parity claim.

The local development DMG and input ZIP remain under ignored
`.cache/macos-dmg-output-20261109/` and `.cache/macos-dmg-input-20261109/`.
This is local ARM64 development-artifact evidence, not an Intel release build,
Gatekeeper/notarization qualification or a published release. No native
application window was launched.

The final focused Python batch passes 54 portable/native-package contract tests
and 12 signature tests, including ten DMG-specific cases. Most tool interactions
in the contract suite are controlled doubles; the separate local native run above
executes actual `dmgbuild`, `hdiutil`, DS_Store reads and filesystem comparisons.
The strengthened final verification also confirms native UDZO format, compression,
checksums and absence of encryption. Source generation is deterministic within the
installed packaging-tool environment; no claim of byte-identical DMGs across
hosts is made because volume identity and native filesystem metadata differ.
