# CLI release preparation

The [release workflow](../../.github/workflows/release.yml) builds the headless
`tablecraft` executable on six native runners. Each runner packages its binary
and tests the extracted `--version` and `--help`. The workflow follows the
portable CLI and complete-release gates used by the local PaintCraft project.
The archives also include optional file-handler integration for `.mdb` and
`.accdb`. The current handler opens inert CLI inspection in a terminal; it is
not the future desktop database editor.

The version is **0.1.0**. Local verification does not push, tag, dispatch a workflow
or publish a release. Manual workflow runs are build-only; publication requires a
separate maintainer-authorized version-tag push.

## Downloads

Names begin `tablecraft-cli-<version>-`, for example
`tablecraft-cli-0.1.0-macos-arm64.tar.gz`.

| Platform | Rust target | Archive suffix |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-x64.zip` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `windows-arm64.zip` |
| macOS Intel | `x86_64-apple-darwin` | `macos-x64.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `macos-arm64.tar.gz` |
| Linux x64 | `x86_64-unknown-linux-gnu` | `linux-x64.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `linux-arm64.tar.gz` |

Each archive contains `tablecraft` (`tablecraft.exe` on Windows), `LICENSE`,
the consolidated `ThirdPartyLicenses.txt`, and `RELEASE.json` with version,
target and payload hashes, plus the platform handler described below.
The executables have Unix mode 0755. Extract
into a directory of your choice and add that directory to PATH.

## File-manager integration

Install into a stable location before registering. These integrations advertise
TableCraft as an optional viewer and do not set the user's default application.
Use the operating system's Open With/default-app settings to choose TableCraft.
Opening a file runs `tablecraft open --wait -- PATH`, which reports header
metadata without running queries, macros, VBA or external connections. Header
inspection does not decrypt the database or need its password; explicit catalog
and row commands prompt when necessary. Handler commands never store passwords.
The terminal waits for Return after inspection or an operation error. Redirected
commands do not wait.

| Platform | Included integration | Installation and removal |
| --- | --- | --- |
| Windows | `register-file-associations.ps1` beside `tablecraft.exe` | Run `powershell -NoProfile -File .\register-file-associations.ps1 -Action Register`; use `-Action Unregister` before moving/removing the installation. No administrator rights are needed. |
| Linux | `file_associations.py` beside `tablecraft` | Run `python3 file_associations.py install`; use `uninstall` before moving/removing it. Requires Python 3, `update-mime-database` and `update-desktop-database`. Files go under `$XDG_DATA_HOME` or `~/.local/share`. |
| macOS | `TableCraft.app`, including its own matching CLI and native AppKit document receiver | Move the app to `~/Applications` or `/Applications` and open it once; select it in Finder's Open With menu. Remove the app to uninstall. The separate portable `tablecraft` remains available for shell use. |

Windows uses per-user OpenWithProgids and application capabilities, without
writing UserChoice or extension defaults. Linux installs an owned desktop entry
with `Terminal=true` and extension MIME declarations, leaving `mimeapps.list`
untouched. Both helpers refuse to overwrite another installation's or a modified
registration. macOS declares a Viewer with Alternate handler rank and receives
Finder document events; the CLI alone cannot receive those events. The bundle
uses Apple's documented legacy extension matching while canonical foreign
MDB/ACCDB UTIs remain unverified, rather than inventing competing type identities.
Its Terminal
bridge passes database paths as process arguments, never as shell expressions.

No registration runs during extraction, packaging, version/help checks or ordinary
database inspection. Native file-manager launch checks remain separate from the
headless request and registration-rendering tests; CI does not change runner
defaults or register handlers.

Each archive has a `.sha256` sidecar. The complete set has fourteen assets:
six archives, six sidecars, one shared `ThirdPartyLicenses.txt` alongside the
archives, and `SHA256SUMS.txt`. The checksum manifest covers all thirteen other
files, excluding itself. These are integrity checks, not
publisher signatures. Windows binaries are not Authenticode-signed; macOS packages
are not Developer ID signed or notarized. Windows builds link the CRT statically;
macOS targets 13.0 or newer. Linux builds use Ubuntu 22.04/glibc 2.35 and require
compatible host runtime libraries.

## Local verification

Use the toolchain in `rust-toolchain.toml` and locked dependencies:

```sh
cargo build --locked --release -p tablecraft-cli --target aarch64-apple-darwin
python3 scripts/packaging/build_macos_app.py --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/tablecraft --output dist-app/TableCraft.app
python3 scripts/packaging/package_cli.py validate
python3 scripts/packaging/package_cli.py package --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/tablecraft \
  --macos-app dist-app/TableCraft.app --output dist
python3 scripts/packaging/package_cli.py verify-archive --version 0.1.0 \
  --target aarch64-apple-darwin \
  --archive dist/tablecraft-cli-0.1.0-macos-arm64.tar.gz --smoke
```

For Windows, set `RUSTFLAGS=-C target-feature=+crt-static` before building and use
the corresponding MSVC target. Smoke checks execute only inert version/help
commands on the native platform. macOS additionally builds the Swift receiver
with the platform SDK and checks its request route without launching Finder or
Terminal. Package creation does not execute its input.

The scripts require the workspace version, a matching tag when supplied,
nonempty release notes and both license/notice files. They check PE/ELF/thin
Mach-O architecture, exact archive contents, payload hashes and executable
permissions. Extra paths, duplicate names, symlinks and stale output files refuse.
Temporary extraction has owned cleanup and does not follow archive paths outside
its directory.

After collecting all six archives, sidecars and the identical shared
`ThirdPartyLicenses.txt` into a fresh directory:

```sh
python3 scripts/packaging/package_cli.py finalize --version 0.1.0 --output dist
python3 scripts/packaging/package_cli.py verify --version 0.1.0 --output dist
(cd dist && sha256sum --check SHA256SUMS.txt)
```

Assembly requires the exact complete set and verifies every archive's version,
architecture, notices, payload manifest and sidecar. It cannot execute foreign
binaries; each native build job must pass its own extracted smoke test first.

## CI and future publication

Pull requests changing packaging or release inputs run the Python tests and the
six native builds. Native labels follow the
[GitHub runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Actions use current official version tags for
[checkout](https://github.com/actions/checkout/releases),
[setup-python](https://github.com/actions/setup-python/releases),
[upload-artifact](https://github.com/actions/upload-artifact/releases) and
[download-artifact](https://github.com/actions/download-artifact/releases).
The workflow uses the repository Rust toolchain without overriding incremental
compilation settings.

A future authorized `v0.1.0` tag must match Cargo and the nonempty
[release notes](../releases/v0.1.0.md). Its isolated publication job runs only after
all builds and complete assembly succeed. It verifies checksums, uploads to a
draft, checks exact asset names and publishes. Existing published versions refuse
replacement. Future prerelease tags set GitHub's prerelease flag; 0.1.0 is stable.
Pull requests and manual runs cannot publish.

```sh
python3 -m unittest discover -s scripts/tests -p 'test_package_*.py'
python3 -m unittest discover -s scripts/tests -p 'test_file_associations.py'
actionlint -shellcheck= .github/workflows/release.yml
```

Synthetic packaging tests, local native smoke checks, remote CI and publication
are separate evidence. Packaging does not change native database compatibility,
writing or VBA limits.
