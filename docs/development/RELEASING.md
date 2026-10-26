# CLI release preparation

The [release workflow](../../.github/workflows/release.yml) builds the headless
`tablecraft` executable on six native runners. Each runner packages its binary
and tests the extracted `--version` and `--help`. The workflow follows the
portable CLI and complete-release gates used by the local PaintCraft project.
There are no GUI installers or system file associations.

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

Each archive contains four files: `tablecraft` (`tablecraft.exe` on Windows),
`LICENSE`, the consolidated `ThirdPartyLicenses.txt`, and `RELEASE.json` with
version, target and payload hashes. The executable has Unix mode 0755. Extract
into a directory of your choice and add that directory to PATH.

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
python3 scripts/packaging/package_cli.py validate
python3 scripts/packaging/package_cli.py package --version 0.1.0 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/tablecraft --output dist
python3 scripts/packaging/package_cli.py verify-archive --version 0.1.0 \
  --target aarch64-apple-darwin \
  --archive dist/tablecraft-cli-0.1.0-macos-arm64.tar.gz --smoke
```

For Windows, set `RUSTFLAGS=-C target-feature=+crt-static` before building and use
the corresponding MSVC target. Smoke checks execute only inert version/help
commands on the native platform. Package creation does not execute its input.

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
actionlint -shellcheck= .github/workflows/release.yml
```

Synthetic packaging tests, local native smoke checks, remote CI and publication
are separate evidence. Packaging does not change native database compatibility,
writing or VBA limits.
