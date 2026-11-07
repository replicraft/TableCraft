#!/usr/bin/env python3
"""Assemble the exact portable desktop asset set without publishing it."""
import argparse
from pathlib import Path
import subprocess
import tarfile
import zipfile
import zlib

import package_desktop as desktop
import release_contract as contract

NOTICES = "DesktopThirdPartyLicenses.txt"
CHECKSUMS = "SHA256SUMS-desktop.txt"
MAX_SIDECAR_BYTES = 4096
MAX_CHECKSUM_BYTES = 64 * 1024


def checksum(path):
    return f"{contract.digest(path)}  {path.name}\n".encode("ascii")


def finalize(version, output, notices, icon, verify_only=False):
    contract.validate_version(version)
    if output.is_symlink() or not output.is_dir():
        raise ValueError("Desktop assembly requires a regular directory")

    archives = {
        desktop.archive_name(version, target) for target in contract.TARGETS
    }
    payloads = archives | {name + ".sha256" for name in archives}
    expected = payloads | ({NOTICES, CHECKSUMS} if verify_only else set())
    actual = {path.name for path in output.iterdir()}
    if actual != expected:
        raise ValueError(
            f"Desktop release set differs: missing={sorted(expected - actual)}, "
            f"extra={sorted(actual - expected)}"
        )

    notice_source = contract.regular(notices, contract.MAX_LEGAL_BYTES)
    desktop.validate_icon(icon)
    for target in contract.TARGETS:
        archive = output / desktop.archive_name(version, target)
        target_icon = icon if "apple" in target else None
        desktop.verify_archive(archive, version, target, notice_source, target_icon)
        sidecar = contract.regular(
            output / (archive.name + ".sha256"), MAX_SIDECAR_BYTES,
        )
        if sidecar.read_bytes() != checksum(archive):
            raise ValueError(f"Desktop checksum sidecar differs: {archive.name}")

    notices_bytes = notice_source.read_bytes()
    bundled_notices = output / NOTICES
    manifest = output / CHECKSUMS
    if verify_only:
        contract.regular(bundled_notices, contract.MAX_LEGAL_BYTES)
        if bundled_notices.read_bytes() != notices_bytes:
            raise ValueError("Standalone desktop notices differ from reviewed input")
        expected_checksums = b"".join(
            checksum(output / name) for name in sorted(payloads | {NOTICES})
        )
        if contract.regular(manifest, MAX_CHECKSUM_BYTES).read_bytes() != expected_checksums:
            raise ValueError("Desktop complete checksum manifest differs")
        return

    # Every archive and sidecar is checked before creating assembly outputs.
    # Remove only files this invocation successfully created if writing fails.
    created = []
    try:
        with bundled_notices.open("xb") as stream:
            created.append(bundled_notices)
            stream.write(notices_bytes)
        checksums = b"".join(
            checksum(output / name) for name in sorted(payloads | {NOTICES})
        )
        with manifest.open("xb") as stream:
            created.append(manifest)
            stream.write(checksums)
    except BaseException:
        for path in reversed(created):
            path.unlink(missing_ok=True)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("finalize", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--version", required=True)
        command.add_argument("--output", type=Path, required=True)
        command.add_argument("--notices", type=Path, required=True)
        command.add_argument("--icon", type=Path, required=True)
    args = parser.parse_args()
    try:
        finalize(
            args.version, args.output, args.notices, args.icon,
            verify_only=args.command == "verify",
        )
    except (
        OSError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile,
        tarfile.TarError, zlib.error,
    ) as error:
        parser.exit(1, f"Desktop release assembly failed: {error}\n")


if __name__ == "__main__":
    main()
